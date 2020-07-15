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
    #[derive(Clone)]
    struct DummyConnectionConnector {}
    impl ConnectionConnector for DummyConnectionConnector {
        type Conn = DummyConnection;

        fn connect(&self) -> Option<Self::Conn> {
            Some(DummyConnection {})
        }
    }
    // #[test]
    // fn connector_works() {
    //     let cc = DummyConnectionConnector {};

    //     let pool = GenericConnectionPool::new(2, cc);
    //     println!("here");
    //     {
    //         for _ in 0..5 {
    //             let pool = pool.clone();
    //             std::thread::spawn(move || {
    //                 let parent_thread_id = thread::current().id();
    //                 println!("threadId: {:?}", parent_thread_id);
    //                 let conn = pool.get_connection().unwrap();
    //                 println!(
    //                     "got conection for parentId: {:?} and now sleeping for 2 seconds",
    //                     parent_thread_id
    //                 );
    //                 thread::sleep(Duration::from_secs(2));
    //                 println!("awoken: {:?}", parent_thread_id);
    //             });
    //         }
    //     }
    //     println!("waiting for 20 seconds");
    //     thread::sleep(Duration::from_secs(20));
    // }

    #[test]
    fn reuse_single_connection() {
        let cc = DummyConnectionConnector {};

        let pool = GenericConnectionPool::new(5, 2, Duration::from_secs(10), cc);
        println!("here");
        {
            for _ in 0..5 {
                let pool = pool.clone();
                std::thread::spawn(move || {
                    let conn = pool.get_connection().unwrap();
                    thread::sleep(Duration::from_secs(1));
                });
                // thread::sleep(Duration::from_secs(2));
            }
        }
        println!("waiting for 20 seconds");
        thread::sleep(Duration::from_secs(25));
        for _ in 0..5 {
            let pool = pool.clone();
            std::thread::spawn(move || {
                let conn = pool.get_connection().unwrap();
                thread::sleep(Duration::from_secs(1));
            });
            // thread::sleep(Duration::from_secs(2));
        }
        println!("waiting for 25 seconds");
        thread::sleep(Duration::from_secs(25));
    }
}
