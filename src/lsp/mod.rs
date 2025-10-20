use std::io::BufReader;

mod diagnostics;
mod documents;
mod handlers;
mod transport;

use handlers::{DispatchAction, LanguageServer};
use transport::read_message;

pub fn run() -> i32 {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();

    let mut reader = BufReader::new(stdin.lock());
    let writer = stdout.lock();
    let mut server = LanguageServer::new(writer);

    loop {
        match read_message(&mut reader) {
            Ok(message) => match server.dispatch(message) {
                DispatchAction::Continue => {}
                DispatchAction::Exit(code) => {
                    server.flush();
                    return code;
                }
            },
            Err(err) => {
                if err.is_eof() {
                    server.flush();
                    return if server.shutdown_requested() { 0 } else { 1 };
                }

                server.log_transport_error(err);
            }
        }
    }
}
