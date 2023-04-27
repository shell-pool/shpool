# MD5 [![Package][package-img]][package-url] [![Documentation][documentation-img]][documentation-url] [![Build][build-img]][build-url]

The package provides the [MD5] hash function.

## Example

```rust
let digest = md5::compute(b"abcdefghijklmnopqrstuvwxyz");
assert_eq!(format!("{:x}", digest), "c3fcd3d76192e4007dfb496cca67e13b");
```

## Security Warning

The package is provided for the purposes of interoperability with protocols and
systems that mandate the use of MD5. However, MD5 should be considered
[cryptographically broken and unsuitable for further use][VU836068]. Collision
attacks against MD5 are both practical and trivial, and [theoretical attacks
against MD5 have been found][ACM1724151].

[RFC6151] advises no new protocols to be designed with any MD5-based
constructions, including HMAC-MD5.

## Contribution

Your contribution is highly appreciated. Do not hesitate to open an issue or a
pull request. Note that any contribution submitted for inclusion in the project
will be licensed according to the terms given in [LICENSE.md](LICENSE.md).

[build-img]: https://travis-ci.org/stainless-steel/md5.svg?branch=master
[build-url]: https://travis-ci.org/stainless-steel/md5
[documentation-img]: https://docs.rs/md5/badge.svg
[documentation-url]: https://docs.rs/md5
[package-img]: https://img.shields.io/crates/v/md5.svg
[package-url]: https://crates.io/crates/md5

[MD5]: https://en.wikipedia.org/wiki/MD5

[ACM1724151]: https://dl.acm.org/citation.cfm?id=1724151
[RFC6151]: https://tools.ietf.org/html/rfc6151
[VU836068]: https://www.kb.cert.org/vuls/id/836068
