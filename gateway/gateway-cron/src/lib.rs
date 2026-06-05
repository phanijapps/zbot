#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]

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
    CreateCronJobRequest, CronHttpAction, CronJobConfig, CronJobKind, CronJobsStore, TriggerResult,
    UpdateCronJobRequest,
};
pub use service::{CronResult, CronService, CronServiceError};
