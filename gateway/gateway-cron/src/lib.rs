#![allow(clippy::missing_docs_in_private_items)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::module_name_repetitions)]
#![allow(missing_docs)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::fn_params_excessive_bools)]
#![allow(clippy::items_after_statements)]
#![allow(clippy::unnecessary_wraps)]
//! # Gateway Cron
//!
//! Cron job configuration and persistence for the AgentZero gateway.
//!
//! This crate provides:
//! - [`CronJobConfig`]: Configuration for scheduled jobs
//! - [`CronService`]: CRUD operations and file-based persistence
//! - Request/response types for the cron API

pub mod config;
pub mod service;

pub use config::{
    CreateCronJobRequest, CronJobConfig, CronJobsStore, TriggerResult, UpdateCronJobRequest,
};
pub use service::{CronResult, CronService, CronServiceError};
