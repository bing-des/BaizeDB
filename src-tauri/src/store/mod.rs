/// 连接配置存储抽象层
///
/// 通过 `ConnectionStore` trait 屏蔽底层存储实现细节（当前为 SQLite）。
/// 后续切换存储方式只需实现此 trait，无需改动 command 层代码。

pub mod connection_store;

pub use connection_store::init_store;
