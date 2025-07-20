use crate::Connection;

pub trait ConnectionLike {
    fn as_connection(&self) -> &Connection;
    fn as_connection_mut(&mut self) -> &mut Connection;
}

impl ConnectionLike for Connection {
    fn as_connection(&self) -> &Connection {
        self
    }
    fn as_connection_mut(&mut self) -> &mut Connection {
        self
    }
}

use super::connection::PoolConnection;

impl ConnectionLike for PoolConnection {
    fn as_connection(&self) -> &Connection {
        self
    }
    fn as_connection_mut(&mut self) -> &mut Connection {
        self
    }
}

impl<T> ConnectionLike for &mut T
where
    T: ConnectionLike + ?Sized,
{
    fn as_connection(&self) -> &Connection {
        (**self).as_connection()
    }

    fn as_connection_mut(&mut self) -> &mut Connection {
        (**self).as_connection_mut()
    }
}
