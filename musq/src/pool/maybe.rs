use crate::{pool::PoolConnection, Connection};

use std::ops::{Deref, DerefMut};

pub enum MaybePoolConnection<'c> {
    Connection(&'c mut Connection),
    PoolConnection(PoolConnection),
}

impl<'c> Deref for MaybePoolConnection<'c> {
    type Target = Connection;

    fn deref(&self) -> &Self::Target {
        match self {
            MaybePoolConnection::Connection(v) => v,
            MaybePoolConnection::PoolConnection(v) => v,
        }
    }
}

impl<'c> DerefMut for MaybePoolConnection<'c> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            MaybePoolConnection::Connection(v) => v,
            MaybePoolConnection::PoolConnection(v) => v,
        }
    }
}

impl<'c> From<PoolConnection> for MaybePoolConnection<'c> {
    fn from(v: PoolConnection) -> Self {
        MaybePoolConnection::PoolConnection(v)
    }
}

impl<'c> From<&'c mut Connection> for MaybePoolConnection<'c> {
    fn from(v: &'c mut Connection) -> Self {
        MaybePoolConnection::Connection(v)
    }
}
