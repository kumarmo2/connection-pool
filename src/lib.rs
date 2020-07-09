use std::{
    mem::{self, MaybeUninit},
    ops::Deref,
    sync::{Arc, Mutex},
};

pub trait ConnectionConnector {
    type Conn: Connection;
    // TODO:  Will need to change the Option to Result
    fn connect(&self) -> Option<Self::Conn>;
}

pub trait Connection {
    fn is_alive(&self) -> bool;
}

pub struct LiveConnection<'a, T>
where
    T: ConnectionConnector,
{
    conn: <T as ConnectionConnector>::Conn,
    pool: &'a GenericConnectionPool<T>,
}

impl<'a, T> Deref for LiveConnection<'a, T>
where
    T: ConnectionConnector,
{
    type Target = T::Conn;
    fn deref(&self) -> &Self::Target {
        &self.conn
    }
}

impl<'a, T> Drop for LiveConnection<'a, T>
where
    T: ConnectionConnector,
{
    fn drop(&mut self) {
        //TODO: Move this logic to GenericConnectionPool.
        let pool = self.pool.get_connections();
        let mut guard = pool.lock().unwrap();
        let x = MaybeUninit::<<T as ConnectionConnector>::Conn>::uninit();
        let x = unsafe { x.assume_init() };
        let real = mem::replace(&mut self.conn, x);
        println!("In drop of LiveConnection");
        guard.push(real);
    }
}

pub struct GenericConnectionPool<E>
where
    E: ConnectionConnector,
{
    _max_connections: u8,
    _min_connections: u8,
    _connections: Arc<Mutex<Vec<<E as ConnectionConnector>::Conn>>>,
    _connector: E,
}

impl<E> GenericConnectionPool<E>
where
    E: ConnectionConnector,
{
    //TODO: Respect the max and min connections constraints.
    pub fn get_connection(&self) -> Option<LiveConnection<E>> {
        let connections = Arc::clone(&self._connections);
        let mut guard = connections.lock().unwrap();
        let conn;
        loop {
            if let 0 = guard.len() {
                println!("making a new connection");
                conn = self._connector.connect().unwrap();
                break;
            }

            let local_conn = guard.pop().unwrap();
            if local_conn.is_alive() {
                println!("reusing connection");
                conn = local_conn;
                break;
            }
        }
        Some(LiveConnection { conn, pool: self })
    }

    fn get_connections(&self) -> Arc<Mutex<Vec<<E as ConnectionConnector>::Conn>>> {
        Arc::clone(&self._connections)
    }
}

#[cfg(test)]
mod tests {
    use super::{Connection, ConnectionConnector, GenericConnectionPool};
    use std::sync::{Arc, Mutex};

    #[test]
    fn connector_works() {
        struct DummyConnection {};

        impl Connection for DummyConnection {
            fn is_alive(&self) -> bool {
                true
            }
        }
        struct DummyConnectionConnector {};
        impl ConnectionConnector for DummyConnectionConnector {
            type Conn = DummyConnection;

            fn connect(&self) -> Option<Self::Conn> {
                Some(DummyConnection {})
            }
        }

        let cc = DummyConnectionConnector {};

        let min = 1;

        let connections: Vec<DummyConnection> = (0..min).map(|_| cc.connect().unwrap()).collect();

        let pool = GenericConnectionPool::<DummyConnectionConnector> {
            _max_connections: 4,
            _min_connections: 1,
            _connections: Arc::new(Mutex::new(connections)),
            _connector: cc,
        };
        {
            let _connection = pool.get_connection();
        }
        {
            let _connection = pool.get_connection();
            let _connection = pool.get_connection();
            let _connection = pool.get_connection();
        }
        {
            let _connection = pool.get_connection();
            let _connection = pool.get_connection();
            let _connection = pool.get_connection();
        }
    }
}
