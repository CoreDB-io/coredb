use pgx::prelude::*;

pgx::pg_module_magic!();

const VT_DEFAULT: i64 = 60;

// TODOs:
// 2. how to make fn param optional with defaults?
// 3. alter queue attributes
// 4. alter message vt
// 5. background worker which updates VT on messages. can this be automatic?

#[pg_extern]
fn psmq_create(queue_name: &str) -> bool {
    Spi::run(&format!(
        "CREATE TABLE {name} (
            msg_id SERIAL,
            vt BIGINT,
            visible BOOL DEFAULT TRUE,
            message JSON
        );",
        name = queue_name
    ));
    true
}

// puts messages onto the queue
#[pg_extern]
fn psmq_enqueue(queue_name: &str, message: pgx::Json) -> Option<i64> {
    Spi::get_one(&format!(
        "INSERT INTO {queue_name} (vt, visible, message)
            VALUES ('{vt}', '{visible}', '{message}'::json)
            RETURNING msg_id;",
        queue_name = queue_name,
        vt = 1,
        visible = true,
        message = message.0,
    ))
}

// check message out of the queue
// TODO: impl a read_many
// how to make pgx function accept optional params?
#[pg_extern]
fn psmq_read(queue_name: &str, vt: Option<i64>) -> pgx::Json {
    let _vt = vt.unwrap_or(VT_DEFAULT);

    let msg = Spi::get_one(&format!(
        "
        WITH cte AS
            (
                SELECT *
                FROM '{queue_name}'
                LIMIT 1
                FOR UPDATE SKIP LOCKED
            )
        UPDATE '{queue_name}'
        SET visible = false, vt = {_vt}
        WHERE rank = (select rank from cte)
        RETURNING *;
        "
    ));
    msg.unwrap()
}

// #[pg_extern]
// fn psmq_delete(queue_name: &str, msg_id: String) -> bool {
//     Spi::run(
//         format!(
//             "
//             DELETE
//             FROM '{queue}'
//             WHERE msg_id = '{msg_id}';
//             ",
//             queue = queue_name,
//             msg_id=msg_id
//         ),
//     )
// };

// reads and deletes at same time
// #[pg_extern]
// fn psmq_pop(queue_name: &str, vt: i64, qty: i32) -> bool {
//     Spi::run(
//         format!(
//             "
//             WITH cte AS
//                 (
//                     SELECT *
//                     FROM '{queue_name}'
//                     LIMIT {qty}
//                     FOR UPDATE SKIP LOCKED
//                 )
//             UPDATE '{queue_name}'
//             DELETE
//             WHERE rank = (select rank from cte)
//             RETURNING *;
//             "
//         ),
//     )
// };

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use pgx::prelude::*;

    #[pg_test]
    fn test_create() {
        let qname = r#"test_queue"#;
        crate::psmq_create(&qname);
        let retval = Spi::get_one::<i32>(&format!("SELECT count(*) FROM {q}", q = &qname))
            .expect("SQL select failed");
        assert_eq!(retval, 0);
        crate::psmq_enqueue(&qname, pgx::Json(serde_json::json!({"x":"y"})));
        let retval = Spi::get_one::<i32>(&format!("SELECT count(*) FROM {q}", q = &qname))
            .expect("SQL select failed");
        assert_eq!(retval, 1);
    }
}

#[cfg(test)]
pub mod pg_test {
    // pg_test module with both the setup and postgresql_conf_options functions are required
    pub fn setup(_options: Vec<&str>) {
        // perform one-off initialization when the pg_test framework starts
    }

    pub fn postgresql_conf_options() -> Vec<&'static str> {
        // return any postgresql.conf settings that are required for your tests
        vec![]
    }
}