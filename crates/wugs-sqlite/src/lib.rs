//! Minimal safe wrapper over SQLite, compiled from source into the binary by
//! `libsqlite3-sys` (`bundled`), so every OS gets the same version with FTS5.
//! Swappable for `rusqlite` later; the surface is deliberately tiny: open,
//! exec, prepare/bind/step/column.

use libsqlite3_sys as _;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;

#[allow(non_camel_case_types)]
type sqlite3 = c_void;
#[allow(non_camel_case_types)]
type sqlite3_stmt = c_void;

extern "C" {
    fn sqlite3_open(filename: *const c_char, db: *mut *mut sqlite3) -> c_int;
    fn sqlite3_close(db: *mut sqlite3) -> c_int;
    fn sqlite3_prepare_v2(
        db: *mut sqlite3,
        sql: *const c_char,
        n: c_int,
        stmt: *mut *mut sqlite3_stmt,
        tail: *mut *const c_char,
    ) -> c_int;
    fn sqlite3_step(stmt: *mut sqlite3_stmt) -> c_int;
    fn sqlite3_finalize(stmt: *mut sqlite3_stmt) -> c_int;
    fn sqlite3_reset(stmt: *mut sqlite3_stmt) -> c_int;
    fn sqlite3_clear_bindings(stmt: *mut sqlite3_stmt) -> c_int;
    // destructor is declared as isize so we can pass SQLITE_TRANSIENT (-1)
    fn sqlite3_bind_text(
        stmt: *mut sqlite3_stmt,
        idx: c_int,
        s: *const c_char,
        n: c_int,
        dtor: isize,
    ) -> c_int;
    fn sqlite3_bind_int64(stmt: *mut sqlite3_stmt, idx: c_int, v: i64) -> c_int;
    fn sqlite3_bind_null(stmt: *mut sqlite3_stmt, idx: c_int) -> c_int;
    fn sqlite3_column_text(stmt: *mut sqlite3_stmt, i: c_int) -> *const u8;
    fn sqlite3_column_bytes(stmt: *mut sqlite3_stmt, i: c_int) -> c_int;
    fn sqlite3_column_int64(stmt: *mut sqlite3_stmt, i: c_int) -> i64;
    fn sqlite3_column_type(stmt: *mut sqlite3_stmt, i: c_int) -> c_int;
    fn sqlite3_errmsg(db: *mut sqlite3) -> *const c_char;
}

const SQLITE_OK: c_int = 0;
const SQLITE_ROW: c_int = 100;
const SQLITE_DONE: c_int = 101;
const SQLITE_NULL: c_int = 5;
const SQLITE_TRANSIENT: isize = -1;

pub struct Db {
    h: *mut sqlite3,
}

// SQLite in serialized/default mode is safe to move between threads.
unsafe impl Send for Db {}

impl Db {
    pub fn open(path: &str) -> Result<Db, String> {
        let c = CString::new(path).map_err(|_| "path contains NUL".to_string())?;
        let mut h: *mut sqlite3 = ptr::null_mut();
        let rc = unsafe { sqlite3_open(c.as_ptr(), &mut h) };
        if rc != SQLITE_OK {
            let msg = if h.is_null() {
                "out of memory opening database".to_string()
            } else {
                let m = errmsg(h);
                unsafe { sqlite3_close(h) };
                m
            };
            return Err(msg);
        }
        Ok(Db { h })
    }

    pub fn open_memory() -> Result<Db, String> {
        Db::open(":memory:")
    }

    fn err(&self) -> String {
        errmsg(self.h)
    }

    /// Execute one or more ';'-separated statements that return no rows.
    pub fn exec(&self, sql: &str) -> Result<(), String> {
        let c = CString::new(sql).map_err(|_| "sql contains NUL".to_string())?;
        let mut p = c.as_ptr();
        let end = unsafe { c.as_ptr().add(sql.len()) };
        while (p as usize) < (end as usize) {
            let mut stmt: *mut sqlite3_stmt = ptr::null_mut();
            let mut tail: *const c_char = ptr::null();
            let rc = unsafe { sqlite3_prepare_v2(self.h, p, -1, &mut stmt, &mut tail) };
            if rc != SQLITE_OK {
                return Err(self.err());
            }
            if stmt.is_null() {
                // trailing whitespace / comments
                p = tail;
                if p.is_null() {
                    break;
                }
                continue;
            }
            loop {
                match unsafe { sqlite3_step(stmt) } {
                    SQLITE_ROW => continue,
                    SQLITE_DONE => break,
                    _ => {
                        let e = self.err();
                        unsafe { sqlite3_finalize(stmt) };
                        return Err(e);
                    }
                }
            }
            unsafe { sqlite3_finalize(stmt) };
            p = tail;
            if p.is_null() {
                break;
            }
        }
        Ok(())
    }

    pub fn prepare(&self, sql: &str) -> Result<Stmt<'_>, String> {
        let c = CString::new(sql).map_err(|_| "sql contains NUL".to_string())?;
        let mut stmt: *mut sqlite3_stmt = ptr::null_mut();
        let rc =
            unsafe { sqlite3_prepare_v2(self.h, c.as_ptr(), -1, &mut stmt, ptr::null_mut()) };
        if rc != SQLITE_OK || stmt.is_null() {
            return Err(format!("prepare failed: {} (sql: {sql})", self.err()));
        }
        Ok(Stmt { h: stmt, db: self })
    }
}

impl Drop for Db {
    fn drop(&mut self) {
        unsafe { sqlite3_close(self.h) };
    }
}

fn errmsg(h: *mut sqlite3) -> String {
    unsafe {
        let p = sqlite3_errmsg(h);
        if p.is_null() {
            "unknown sqlite error".to_string()
        } else {
            CStr::from_ptr(p).to_string_lossy().into_owned()
        }
    }
}

/// SQLite takes text lengths as `int`; a longer string would wrap negative and
/// make it read out of bounds.
fn checked_text_len(len: usize) -> Result<c_int, String> {
    c_int::try_from(len).map_err(|_| format!("text of {len} bytes exceeds SQLite's 2 GiB limit"))
}

pub struct Stmt<'a> {
    h: *mut sqlite3_stmt,
    db: &'a Db,
}

impl<'a> Stmt<'a> {
    pub fn bind_text(&mut self, idx: usize, s: &str) -> Result<(), String> {
        let len = checked_text_len(s.len())?;
        let rc = unsafe {
            sqlite3_bind_text(
                self.h,
                idx as c_int,
                s.as_ptr() as *const c_char,
                len,
                SQLITE_TRANSIENT,
            )
        };
        if rc != SQLITE_OK {
            return Err(self.db.err());
        }
        Ok(())
    }

    pub fn bind_opt_text(&mut self, idx: usize, s: Option<&str>) -> Result<(), String> {
        match s {
            Some(s) => self.bind_text(idx, s),
            None => self.bind_null(idx),
        }
    }

    pub fn bind_i64(&mut self, idx: usize, v: i64) -> Result<(), String> {
        let rc = unsafe { sqlite3_bind_int64(self.h, idx as c_int, v) };
        if rc != SQLITE_OK {
            return Err(self.db.err());
        }
        Ok(())
    }

    pub fn bind_null(&mut self, idx: usize) -> Result<(), String> {
        let rc = unsafe { sqlite3_bind_null(self.h, idx as c_int) };
        if rc != SQLITE_OK {
            return Err(self.db.err());
        }
        Ok(())
    }

    /// Returns true if a row is available, false when done.
    pub fn step(&mut self) -> Result<bool, String> {
        match unsafe { sqlite3_step(self.h) } {
            SQLITE_ROW => Ok(true),
            SQLITE_DONE => Ok(false),
            _ => Err(self.db.err()),
        }
    }

    /// Reset for re-execution (keeps bindings unless cleared).
    pub fn reset(&mut self) {
        unsafe {
            sqlite3_reset(self.h);
            sqlite3_clear_bindings(self.h);
        }
    }

    pub fn col_is_null(&self, i: usize) -> bool {
        unsafe { sqlite3_column_type(self.h, i as c_int) == SQLITE_NULL }
    }

    pub fn col_text(&self, i: usize) -> String {
        self.col_opt_text(i).unwrap_or_default()
    }

    pub fn col_opt_text(&self, i: usize) -> Option<String> {
        unsafe {
            let p = sqlite3_column_text(self.h, i as c_int);
            if p.is_null() {
                return None;
            }
            let n = sqlite3_column_bytes(self.h, i as c_int) as usize;
            let bytes = std::slice::from_raw_parts(p, n);
            Some(String::from_utf8_lossy(bytes).into_owned())
        }
    }

    pub fn col_i64(&self, i: usize) -> i64 {
        unsafe { sqlite3_column_int64(self.h, i as c_int) }
    }
}

impl<'a> Drop for Stmt<'a> {
    fn drop(&mut self) {
        unsafe { sqlite3_finalize(self.h) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    extern "C" {
        fn sqlite3_libversion_number() -> c_int;
    }

    #[test]
    fn linked_sqlite_is_the_bundled_one() {
        let v = unsafe { sqlite3_libversion_number() };
        assert_eq!(
            v,
            libsqlite3_sys::SQLITE_VERSION_NUMBER,
            "linked a system SQLite"
        );
        assert!(v >= 3_040_000, "SQLite {v} is older than 3.40");
    }

    #[test]
    fn text_longer_than_int_max_is_refused() {
        assert_eq!(checked_text_len(i32::MAX as usize), Ok(i32::MAX));
        assert!(checked_text_len(i32::MAX as usize + 1).is_err());
    }

    #[test]
    fn fts5_unicode61_folds_diacritics() {
        let db = Db::open_memory().unwrap();
        db.exec(
            "CREATE VIRTUAL TABLE t USING fts5(x, tokenize='unicode61 remove_diacritics 2');
             INSERT INTO t(x) VALUES ('Ṣe ìwọ wá'), ('unrelated');",
        )
        .unwrap();
        let mut q = db.prepare("SELECT x FROM t WHERE t MATCH ?1").unwrap();
        q.bind_text(1, "iwo").unwrap();
        assert!(q.step().unwrap());
        assert_eq!(q.col_text(0), "Ṣe ìwọ wá");
        assert!(!q.step().unwrap());
    }

    #[test]
    fn basic_crud() {
        let db = Db::open_memory().unwrap();
        db.exec("CREATE TABLE t(a TEXT PRIMARY KEY, b INTEGER); CREATE INDEX i ON t(b);")
            .unwrap();
        {
            let mut s = db.prepare("INSERT INTO t(a,b) VALUES(?1,?2)").unwrap();
            for i in 0..100 {
                s.bind_text(1, &format!("key-{i}")).unwrap();
                s.bind_i64(2, i).unwrap();
                assert!(!s.step().unwrap());
                s.reset();
            }
        }
        let mut q = db
            .prepare("SELECT a, b FROM t WHERE b >= ?1 ORDER BY b")
            .unwrap();
        q.bind_i64(1, 97).unwrap();
        let mut rows = Vec::new();
        while q.step().unwrap() {
            rows.push((q.col_text(0), q.col_i64(1)));
        }
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0], ("key-97".to_string(), 97));
    }

    #[test]
    fn null_handling_and_unicode() {
        let db = Db::open_memory().unwrap();
        db.exec("CREATE TABLE t(a TEXT, b TEXT)").unwrap();
        let mut s = db.prepare("INSERT INTO t(a,b) VALUES(?1,?2)").unwrap();
        s.bind_text(1, "köpek-ler ✓").unwrap();
        s.bind_null(2).unwrap();
        s.step().unwrap();
        let mut q = db.prepare("SELECT a, b FROM t").unwrap();
        assert!(q.step().unwrap());
        assert_eq!(q.col_text(0), "köpek-ler ✓");
        assert!(q.col_is_null(1));
        assert_eq!(q.col_opt_text(1), None);
    }

    #[test]
    fn error_reporting() {
        let db = Db::open_memory().unwrap();
        assert!(db.exec("NOT VALID SQL").is_err());
        assert!(db.prepare("SELECT * FROM missing_table").is_err());
    }
}
