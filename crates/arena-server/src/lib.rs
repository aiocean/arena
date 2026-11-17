//! The arena match server, as a library: fighting a match and turning it into `arena-link` frames (`match_run`), shared
//! by the `arena-server` publisher and the `arena-watch` checker; pacing it to the wall clock (`pace`); writing it onto
//! its MoQ track and ending that cleanly (`publish`).

pub mod match_run;
pub mod pace;
pub mod publish;
