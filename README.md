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

## 结构化字段

文档除正文外还可以携带类型化字段。`Schema` 声明字段名、字段类型（`FieldType::Text` / `Integer` / `Boolean`）和是否必需；`FieldValue` 保存对应的文本、整数或布尔值。调用方用 `Document::with_field` 构造带字段文档，用 `Document::field` 按名读取值，用 `SearchIndex::insert_with_schema` 校验并写入。

```rust
use strata_search::{Document, FieldSchema, FieldType, Schema, SearchIndex};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut schema = Schema::new();
    schema.add_field(FieldSchema::new("title", FieldType::Text, true));
    schema.add_field(FieldSchema::new("year", FieldType::Integer, false));

    let mut index = SearchIndex::new();
    let doc = Document::new("storage-guide", "Rust storage guide")?
        .with_field("title", "Storage Guide")
        .with_field("year", 2026_i64);
    index.insert_with_schema(doc, &schema)?;
    Ok(())
}
```

字段写入约定：

- `insert_with_schema` 先完整校验再修改索引，校验失败时索引保持原状。
- 三类校验错误以不同的 `SchemaError` 变体返回：未知字段（`UnknownField`）、必需字段缺失（`MissingField`）、类型不匹配（`TypeMismatch`，含期望与实际类型）。
- 如果同键文档已存在，任何校验失败都不会替换旧文档；`get`、`len`、`remove` 和 `search` 的结果保持失败前状态。
- 校验通过时与 `insert` 一样替换同键文档并返回旧文档。
- 不带字段的文档仍可用 `insert` 直接写入，不做任何 schema 校验；字段不参与词项检索。

## 开发

使用 `rust-toolchain.toml` 指定的 Rust 1.98.1 工具链及系统链接器。项目没有第三方运行依赖。

```bash
cargo test --locked
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

结构说明见 [设计说明](docs/design.md)。
