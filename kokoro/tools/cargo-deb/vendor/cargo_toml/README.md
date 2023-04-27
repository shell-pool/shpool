# Deserialize `Cargo.toml`

<img src="the-real-tom.jpeg" align="left" alt="due to a milkshake duck situation, the preferred Tom for this format has been replaced">

This is a definition of fields in `Cargo.toml` files for [serde](https://serde.rs). It allows reading of `Cargo.toml` data, and serializing it using TOML or other formats. It's used by [the lib.rs site](https://lib.rs) to extract information about crates.

To get started, see [`Manifest::from_slice`][docs].

[docs]: https://docs.rs/cargo_toml/latest/cargo_toml/struct.Manifest.html#method.from_slice

Additionally, this crate supports basic post-processing of the data to emulate Cargo's `autobins` feature, which sets manifest defaults based on presence of files on disk (other non-disk data sources are also supported).

