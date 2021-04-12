use std::{io, time::Duration};

use crossterm::{
    cursor,
    event::{poll, read, Event, KeyCode, KeyEvent},
    execute, queue,
    style::{self, Colorize},
    Result as CResult,
};

pub fn read_key() -> CResult<KeyEvent> {
    loop {
        // Wait up to 1s for another event
        if poll(Duration::from_millis(1_000))? {
            // It's guaranteed that read() wont block if `poll` returns `Ok(true)`
            let event = read()?;

            if let Event::Key(c) = event {
                return Ok(c);
            }
        }
    }
}

pub fn read_line() -> CResult<String> {
    let mut text = String::new();
    loop {
        let key = read_key()?;
        if let KeyCode::Char(c) = key.code {
            text.push(c);
        }

        if key == KeyCode::Enter.into() {
            break;
        }
    }

    Ok(text)
}

pub fn rectangle(stdout: &mut io::Stdout, a: u16, b: u16, w: u16, h: u16) -> CResult<()> {
    for y in 0..h {
        for x in 0..w {
            if (y == b || y == h - 1) || (x == a || x == w - 1) {
                // in this loop we are more efficient by not flushing the buffer.
                queue!(
                    stdout,
                    cursor::MoveTo(x, y),
                    style::PrintStyledContent(" ".on_dark_grey())
                )?;
            }
        }
    }
    Ok(())
}
