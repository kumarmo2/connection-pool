// TODO: Need to properly setup the test module.

/*
* Tests
*
1 Getting connections work.
2 Max Connection respected.
3 Min Connection respected.
4 Idle Connections getting killed
*/

#[cfg(test)]
mod tests {
    use connection_pool::{Connection, ConnectionConnector, GenericConnectionPool};
    use std::thread;
    use std::time::Duration;

    struct DummyConnection {}

    impl Connection for DummyConnection {
        fn is_alive(&self) -> bool {
            true
        }
    }
    struct DummyConnectionConnector {}
    impl ConnectionConnector for DummyConnectionConnector {
        type Conn = DummyConnection;

        fn connect(&self) -> Option<Self::Conn> {
            Some(DummyConnection {})
        }
    }

    #[test]
    fn reuse_single_connection() {
        let cc = DummyConnectionConnector {};

        let pool = GenericConnectionPool::new(20, 2, Duration::from_secs(5), cc);
        println!("here");
        {
            for i in 0..20 {
                let pool = pool.clone();
                std::thread::spawn(move || {
                    let conn = pool.get_connection().unwrap();
                    thread::sleep(Duration::from_secs(i + 5));
                });
                // thread::sleep(Duration::from_secs(2));
            }
        }
        thread::sleep(Duration::from_secs(100));
    }
}
