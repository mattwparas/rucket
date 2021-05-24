#[cfg(not(target_os = "windows"))]
pub const PRELUDE: &str = include_str!("scheme/stdlib.rkt");
// pub const PRELUDE: &str = include_str!("scheme/test.rkt");
#[cfg(not(target_os = "windows"))]
pub const TRIESORT: &str = include_str!("scheme/trie.rkt");
#[cfg(not(target_os = "windows"))]
pub const CONTRACTS: &str = include_str!("scheme/contract.rkt");
#[cfg(not(target_os = "windows"))]
pub const TYPES: &str = include_str!("scheme/types.rkt");
#[cfg(not(target_os = "windows"))]
pub const METHODS: &str = include_str!("scheme/methods.rkt");
#[cfg(not(target_os = "windows"))]
pub const MERGE: &str = include_str!("scheme/merge.rkt");
#[cfg(not(target_os = "windows"))]
pub const COMPILER: &str = include_str!("scheme/nanopass.rkt");
#[cfg(not(target_os = "windows"))]
pub const DISPLAY: &str = include_str!("scheme/display.rkt");

#[cfg(target_os = "windows")]
pub const PRELUDE: &str = include_str!(r#"scheme\stdlib.rkt"#);
#[cfg(target_os = "windows")]
pub const TRIESORT: &str = include_str!(r#"scheme\trie.rkt"#);
#[cfg(target_os = "windows")]
pub const CONTRACTS: &str = include_str!(r#"scheme\contract.rkt"#);
#[cfg(target_os = "windows")]
pub const TYPES: &str = include_str!(r#"scheme\types.rkt"#);
#[cfg(target_os = "windows")]
pub const MERGE: &str = include_str!(r#"scheme\merge.rkt"#);
#[cfg(target_os = "windows")]
pub const COMPILER: &str = include_str!(r#"scheme\nanopass.rkt"#);
#[cfg(target_os = "windows")]
pub const DISPLAY: &str = include_str!("scheme/display.rkt");
