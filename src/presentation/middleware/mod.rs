//! Presentation middleware (hand-written; user-owned).
//!
//! Empty since the tenant-agnostic strip: the former module gate bundled an
//! admin check with a sentinel `app.company_id` scope wrap for fence-none apps
//! composing the then-company-fenced module. The module no longer ships a scoping
//! column or fence (composition-installed tenancy, ADR-0029) — identity and
//! permission gating are the composing service's org auth layer, which mounts
//! over these routes and supplies the ambient request scope.
