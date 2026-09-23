//! Core March α₅ library primitives shared by the CLI and, eventually, the Forth surface.

pub mod builder;
pub mod cbor;
pub mod cid;
pub mod db;
pub mod effect;
pub mod exec;
pub mod global_store;
pub mod guard;
pub mod iface;
pub mod inet;
pub mod interp;
pub mod namespace;
pub mod node;
pub mod prim;
pub mod sexpr;
pub mod surface;
pub mod types;
pub mod word;
pub mod yaml;

pub type Result<T> = anyhow::Result<T>;

pub use builder::{DispatchSpec, GraphBuilder};
pub use db::{
    NameEntry, count_objects_of_kind, create_store, derive_db_path, ensure_parent_dirs, get_name,
    list_names, list_names_for_cid, load_all_cbor_for_kind, load_cbor_for_kind, load_object_cbor,
    open_store, put_name,
};
pub use effect::{EffectCanon, EffectStoreOutcome};
pub use global_store::{
    GlobalStore, GlobalStoreSnapshot, GlobalStoreStoreOutcome, load_snapshot, store_snapshot,
};
pub use guard::{GuardCanon, GuardInfo, GuardStoreOutcome};
pub use iface::{IfaceCanon, IfaceStoreOutcome, IfaceSymbol};
pub use inet::{AgentCanon as InetAgentCanon, Net as InetNet, RuleCanon as InetRuleCanon};
pub use interp::{Value, run_word, run_word_i64};
pub use namespace::{NamespaceCanon, NamespaceExport, NamespaceStoreOutcome};
pub use node::{NodeCanon, NodeInput, NodeKind, NodePayload, NodeStoreOutcome};
pub use prim::{PrimCanon, PrimInfo, PrimStoreOutcome};
pub use sexpr::{SExpr, parse as parse_sexpr, parse_sequence as parse_sexpr_sequence};
pub use surface::{
    AgentSpec, CatalogEntry, Definition, EffectSpec, GuardSpec as SurfaceGuardSpec, InterfaceSpec,
    InterfaceSymbolSpec, NamespaceExportSpec, NamespaceSpec, OverloadEntry, OverloadSetSpec,
    PrimSpec as SurfacePrimSpec, RuleSpec as SurfaceRuleSpec, StackOp as SurfaceStackOp, StateSpec,
    WordSpec as SurfaceWordSpec, parse_catalog_from_sexpr_str, parse_definitions_from_sexpr,
};
pub use types::TypeTag;
pub use word::{WordCanon, WordStoreOutcome};
