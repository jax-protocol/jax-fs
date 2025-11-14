#[macro_use]
mod macros;
pub mod ping;

pub use ping::Ping;

// Register all bidirectional message handlers
// To add a new message type, just add a line here:
//   NewMessage(NewMessageHandler),
register_handlers! {
    Ping(Ping),
}
