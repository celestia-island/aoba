use anyhow::Result;

use crate::protocol::status::{
    read_status,
    types::{self, cursor, Page},
    write_status,
};

pub fn handle_move_prev(cursor: cursor::EntryCursor) -> Result<()> {
    match cursor {
        cursor::EntryCursor::Com { index } => {
            let prev = index.saturating_sub(1);
            write_status(|status| {
                status.page = Page::Entry {
                    cursor: Some(types::cursor::EntryCursor::Com { index: prev }),
                };
                Ok(())
            })?;
        }
        cursor::EntryCursor::Refresh => {
            let prev = read_status(|status| Ok(status.ports.map.len().saturating_sub(1)))?;
            if read_status(|status| Ok(status.ports.map.is_empty()))? {
                write_status(|status| {
                    status.page = Page::Entry {
                        cursor: Some(types::cursor::EntryCursor::Refresh),
                    };
                    Ok(())
                })?;
            } else {
                write_status(|status| {
                    status.page = Page::Entry {
                        cursor: Some(types::cursor::EntryCursor::Com { index: prev }),
                    };
                    Ok(())
                })?;
            }
        }
        cursor::EntryCursor::CreateVirtual => {
            write_status(|status| {
                status.page = Page::Entry {
                    cursor: Some(types::cursor::EntryCursor::Refresh),
                };
                Ok(())
            })?;
        }
        cursor::EntryCursor::About => {
            write_status(|status| {
                status.page = Page::Entry {
                    cursor: Some(types::cursor::EntryCursor::CreateVirtual),
                };
                Ok(())
            })?;
        }
    }

    Ok(())
}

pub fn handle_move_next(cursor: cursor::EntryCursor) -> Result<()> {
    match cursor {
        cursor::EntryCursor::Com { index } => {
            let next = index.saturating_add(1);
            if next >= read_status(|status| Ok(status.ports.map.len()))? {
                write_status(|status| {
                    status.page = Page::Entry {
                        cursor: Some(types::cursor::EntryCursor::Refresh),
                    };
                    Ok(())
                })?;
            } else {
                write_status(|status| {
                    status.page = Page::Entry {
                        cursor: Some(types::cursor::EntryCursor::Com { index: next }),
                    };
                    Ok(())
                })?;
            }
        }
        cursor::EntryCursor::Refresh => {
            write_status(|status| {
                status.page = Page::Entry {
                    cursor: Some(types::cursor::EntryCursor::CreateVirtual),
                };
                Ok(())
            })?;
        }
        cursor::EntryCursor::CreateVirtual => {
            write_status(|status| {
                status.page = Page::Entry {
                    cursor: Some(types::cursor::EntryCursor::About),
                };
                Ok(())
            })?;
        }
        cursor::EntryCursor::About => {}
    }

    Ok(())
}
