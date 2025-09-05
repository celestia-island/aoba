mod auto_slave; // auto slave response + debug helpers
mod edit; // form editing / init / pause-resume helpers
mod logging; // log related structs & helpers
mod polling; // polling driver for master/slave register logic
mod port; // per-port state & port level helpers
mod runtime_events; // draining runtime events & auto slave response
mod scan; // scanning & refresh logic
mod types_base; // core enums & Status struct
mod types_form; // SubpageForm
mod types_register; // RegisterEntry & PendingRequest
mod util; // small utilities (crc, helpers) // moved tests

pub use logging::*;
pub use types_base::*;
pub use types_form::*;
pub use types_register::*;
pub use util::*;
