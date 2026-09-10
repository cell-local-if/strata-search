# Strata Search

Strata Search 是一个可嵌入的文档检索库，面向结构化文本的持续写入与查询。

当前版本提供内存文档存储、稳定文档键、替换和删除，以及基于词项交集的检索。查询扫描现有文档，结果按文档键升序排列。所有数据保存在内存中，关闭程序后不会保留。

## 使用

```rust
use strata_search::{Document, SearchIndex};

fn main() -> Result<(), strata_search::DocumentError> {
    let mut index = SearchIndex::new();
    index.insert(Document::new("storage-guide", "Rust storage guide")?);
    let hits = index.search("rust guide");
    assert_eq!(hits[0].id(), "storage-guide");
    Ok(())
}
```

## 当前检索约定

- 文档键不能为空或全为空白；合法键和正文保留原样。
- 相同键的写入替换整个文档，并返回旧文档。
- 正文和查询按 Unicode 空白切分，对每个词项做 Unicode 小写转换。
- 查询中的所有词项都需要出现，重复词项不会重复返回文档。
- 空查询不匹配任何文档；标点保留在词项中，暂不做重音消除或短语分析。
- 文档键区分大小写，结果使用键的字典序；当前没有相关性评分。

## 开发

使用 `rust-toolchain.toml` 指定的 Rust 1.98.1 工具链及系统链接器。项目没有第三方运行依赖。

```bash
cargo test --locked
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

结构说明见 [设计说明](docs/design.md)。
