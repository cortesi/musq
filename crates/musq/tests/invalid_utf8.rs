use libsqlite3_sys as ffi;
use musq::query;
use musq_test::connection;
use std::ffi::CString;
use std::os::raw::{c_char, c_int};

#[tokio::test]
async fn invalid_utf8_error_message_does_not_panic() -> anyhow::Result<()> {
    let mut conn = connection().await?;

    {
        let mut locked = conn.lock_handle().await?;
        let db = locked.as_raw_handle().as_ptr();
        unsafe extern "C" fn badfunc(
            ctx: *mut ffi::sqlite3_context,
            _argc: c_int,
            _argv: *mut *mut ffi::sqlite3_value,
        ) {
            let msg: &[u8] = b"bad\xffmessage";
            unsafe {
                ffi::sqlite3_result_error(ctx, msg.as_ptr() as *const c_char, msg.len() as c_int);
            }
        }
        let name = CString::new("badfunc").unwrap();
        let rc = unsafe {
            ffi::sqlite3_create_function_v2(
                db,
                name.as_ptr(),
                0,
                ffi::SQLITE_UTF8,
                std::ptr::null_mut(),
                Some(badfunc),
                None,
                None,
                None,
            )
        };
        assert_eq!(rc, ffi::SQLITE_OK);
    }

    let res = query("SELECT badfunc()").execute(&mut conn).await;
    assert!(res.is_err());
    Ok(())
}

#[tokio::test]
async fn invalid_utf8_column_decltype_does_not_panic() -> anyhow::Result<()> {
    let mut conn = connection().await?;
    {
        let mut locked = conn.lock_handle().await?;
        let db = locked.as_raw_handle().as_ptr();
        let sql = CString::new(b"CREATE TABLE t(col T\xff);".to_vec()).unwrap();
        let rc = unsafe {
            ffi::sqlite3_exec(
                db,
                sql.as_ptr(),
                None,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert_eq!(rc, ffi::SQLITE_OK);
    }

    let rows = query("SELECT col FROM t").fetch_all(&mut conn).await?;
    assert!(rows.is_empty());
    Ok(())
}
