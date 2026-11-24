//! fid — information fidelity measurement.
//!
//! fid measures how far stored data still corresponds to the reality it
//! represents. Fidelity decays with age, through lossy transformation, and
//! as the world changes. fid stamps content with a fidelity envelope
//! ([`stamp`]), scores it later ([`check`], [`chain`]), and fits decay from
//! facts that were checked again ([`calibrate`], [`drift`]).
//!
//! The normative definition is spec/draft-kumar-fid-00.md in the repository;
//! the command-line tool is `fid`, published as the `fidmeter` crate.

// Every public item is documented (Rust API Guidelines C-CRATE-DOC;
// eng-principles ENG-STD-008 `docs`). CI runs clippy with -D warnings.
#![warn(missing_docs)]

pub mod calibrate;
pub mod chain;
pub mod check;
pub mod compute;
pub mod drift;
pub mod echo;
pub mod envelope;
pub mod fast;
pub mod jcs;
pub mod registry;
pub mod stamp;
