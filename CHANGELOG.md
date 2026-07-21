# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.47.1](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.47.0...geo-polygonize-v0.47.1) (2026-07-21)


### Bug Fixes

* **core:** gate Parquet behind GeoParquet feature ([#892](https://github.com/graydonpleasants/geo-polygonize/issues/892)) ([2e74a0a](https://github.com/graydonpleasants/geo-polygonize/commit/2e74a0a5a92d16b04eddaefeed9a95a02559848d))

## [0.47.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.46.2...geo-polygonize-v0.47.0) (2026-07-21)


### Features

* **core:** report tiled polygonization outcomes ([#890](https://github.com/graydonpleasants/geo-polygonize/issues/890)) ([ade2a49](https://github.com/graydonpleasants/geo-polygonize/commit/ade2a4927890695ab349a7fd58881caba3601fe3))

## [0.46.2](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.46.1...geo-polygonize-v0.46.2) (2026-07-21)


### Bug Fixes

* **core:** make tiled dedup collision-safe ([#888](https://github.com/graydonpleasants/geo-polygonize/issues/888)) ([49b4402](https://github.com/graydonpleasants/geo-polygonize/commit/49b4402a439e00e0cf1133574fad0a4d438f2edc))

## [0.46.1](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.46.0...geo-polygonize-v0.46.1) (2026-07-21)


### Bug Fixes

* **core:** canonicalize tiled output ordering ([#886](https://github.com/graydonpleasants/geo-polygonize/issues/886)) ([7c5bfbb](https://github.com/graydonpleasants/geo-polygonize/commit/7c5bfbb61bf3d6de9baabd27048af045c3daf6e9))

## [0.46.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.45.1...geo-polygonize-v0.46.0) (2026-07-21)


### Features

* **core:** harden tiled polygonization contract ([#884](https://github.com/graydonpleasants/geo-polygonize/issues/884)) ([e7746dd](https://github.com/graydonpleasants/geo-polygonize/commit/e7746dde4a19e12ab999a8c2ba4459927a07ffe5))

## [0.45.1](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.45.0...geo-polygonize-v0.45.1) (2026-07-21)


### Bug Fixes

* **github:** publish crates without passing registry token inline ([#882](https://github.com/graydonpleasants/geo-polygonize/issues/882)) ([9e09dd0](https://github.com/graydonpleasants/geo-polygonize/commit/9e09dd08f40c1da004c01159645d865b8ba24a42))

## [0.45.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.44.0...geo-polygonize-v0.45.0) (2026-07-21)


### Features

* **core:** make Z handling explicit ([#880](https://github.com/graydonpleasants/geo-polygonize/issues/880)) ([11f8e1e](https://github.com/graydonpleasants/geo-polygonize/commit/11f8e1e4d11e8991e39134f999d7170d6d9a7d0c))

## [0.44.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.43.0...geo-polygonize-v0.44.0) (2026-07-21)


### Features

* **core:** add GeoRust polygonization facade ([#878](https://github.com/graydonpleasants/geo-polygonize/issues/878)) ([3425637](https://github.com/graydonpleasants/geo-polygonize/commit/3425637356d5eb334bfb10fa62810185974aeb3d))

## [0.43.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.42.0...geo-polygonize-v0.43.0) (2026-07-21)


### Features

* **core:** preserve provenance through edge dissolve ([#875](https://github.com/graydonpleasants/geo-polygonize/issues/875)) ([3338276](https://github.com/graydonpleasants/geo-polygonize/commit/33382763b6115e90ebcae542da7f911cec6ae0fc))

## [0.42.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.41.0...geo-polygonize-v0.42.0) (2026-07-21)


### Features

* **core:** add certified hot-pixel noding ([#873](https://github.com/graydonpleasants/geo-polygonize/issues/873)) ([bbab5ed](https://github.com/graydonpleasants/geo-polygonize/commit/bbab5ed3c5f92468041687ee38536a2388e14529))

## [0.41.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.40.1...geo-polygonize-v0.41.0) (2026-07-20)


### Features

* **core:** add explicit precision models ([#869](https://github.com/graydonpleasants/geo-polygonize/issues/869)) ([cd0ca0f](https://github.com/graydonpleasants/geo-polygonize/commit/cd0ca0fa85cd9160ef6bee694ee42439178dc07b))
* **core:** add full-noding validation ([#871](https://github.com/graydonpleasants/geo-polygonize/issues/871)) ([514e5ca](https://github.com/graydonpleasants/geo-polygonize/commit/514e5ca56569f4e08c386a51972562aa44b72abd))

## [0.40.1](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.40.0...geo-polygonize-v0.40.1) (2026-07-20)


### Bug Fixes

* **core:** stabilize containment benchmark contract ([#868](https://github.com/graydonpleasants/geo-polygonize/issues/868)) ([cb4f707](https://github.com/graydonpleasants/geo-polygonize/commit/cb4f707018385ed0e9ecc4e82ee5143ebc757ed4))

## [0.40.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.39.13...geo-polygonize-v0.40.0) (2026-07-20)


### Features

* **core:** harden polygonization API and correctness ([#866](https://github.com/graydonpleasants/geo-polygonize/issues/866)) ([4ae1097](https://github.com/graydonpleasants/geo-polygonize/commit/4ae10973b2cdb2362a19c95bfe5c0a75a70c36ca))

## [0.39.13](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.39.12...geo-polygonize-v0.39.13) (2026-07-19)


### Bug Fixes

* **github:** skip release benchmark refreshes ([#860](https://github.com/graydonpleasants/geo-polygonize/issues/860)) ([a8ce615](https://github.com/graydonpleasants/geo-polygonize/commit/a8ce61561c7c1d8e94fced7aad6ac3633a94cc62))

## [0.39.12](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.39.11...geo-polygonize-v0.39.12) (2026-07-19)


### Performance Improvements

* **core:** dispatch dense noding samples to SIMD ([#854](https://github.com/graydonpleasants/geo-polygonize/issues/854)) ([dd711cc](https://github.com/graydonpleasants/geo-polygonize/commit/dd711ccb71d6dcf910b11dea7c2c9d472f72ed19))

## [0.39.11](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.39.10...geo-polygonize-v0.39.11) (2026-07-19)


### Bug Fixes

* **core:** bound grid traversal at segment endpoints ([#853](https://github.com/graydonpleasants/geo-polygonize/issues/853)) ([f17bfaa](https://github.com/graydonpleasants/geo-polygonize/commit/f17bfaadef29c71f38dfeb83841270d26ab362d4))

## [0.39.10](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.39.9...geo-polygonize-v0.39.10) (2026-07-19)


### Performance Improvements

* **core:** prefer scalar point location on Linux ARM ([#849](https://github.com/graydonpleasants/geo-polygonize/issues/849)) ([d4f6040](https://github.com/graydonpleasants/geo-polygonize/commit/d4f6040b664d29a1100f40ac05b1f58d0607db8f))

## [0.39.9](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.39.8...geo-polygonize-v0.39.9) (2026-07-19)


### Performance Improvements

* **core:** skip small-ring query atomics ([#846](https://github.com/graydonpleasants/geo-polygonize/issues/846)) ([68db8f8](https://github.com/graydonpleasants/geo-polygonize/commit/68db8f8275342d1d32ebb2015284a8f760c12463))

## [0.39.8](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.39.7...geo-polygonize-v0.39.8) (2026-07-19)


### Performance Improvements

* **wasm:** profile polygonization stages ([#840](https://github.com/graydonpleasants/geo-polygonize/issues/840)) ([f7284e1](https://github.com/graydonpleasants/geo-polygonize/commit/f7284e1cd95901ada4f1cb283062fc30ed184ad0))

## [0.39.7](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.39.6...geo-polygonize-v0.39.7) (2026-07-19)


### Performance Improvements

* **wasm:** measure threaded crossover workloads ([#836](https://github.com/graydonpleasants/geo-polygonize/issues/836)) ([7abcdc8](https://github.com/graydonpleasants/geo-polygonize/commit/7abcdc8d62def351179a434816ffa733d5013934))

## [0.39.6](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.39.5...geo-polygonize-v0.39.6) (2026-07-19)


### Bug Fixes

* **wasm:** validate threaded builds across architectures ([#832](https://github.com/graydonpleasants/geo-polygonize/issues/832)) ([dd9e9f3](https://github.com/graydonpleasants/geo-polygonize/commit/dd9e9f3d7dca3586069f419199e7fe31ea5edd89))

## [0.39.5](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.39.4...geo-polygonize-v0.39.5) (2026-07-19)


### Performance Improvements

* **core:** reduce polygonization staging allocations ([#830](https://github.com/graydonpleasants/geo-polygonize/issues/830)) ([e08ea17](https://github.com/graydonpleasants/geo-polygonize/commit/e08ea17b75fa1a9fea9b8d497e68f52f18bca5f4))

## [0.39.4](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.39.3...geo-polygonize-v0.39.4) (2026-07-18)


### Performance Improvements

* **core:** reduce parallel split merge allocations ([#827](https://github.com/graydonpleasants/geo-polygonize/issues/827)) ([b4edbd1](https://github.com/graydonpleasants/geo-polygonize/commit/b4edbd1b565744e24429a90716cd1a26f2f61c5c))

## [0.39.3](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.39.2...geo-polygonize-v0.39.3) (2026-07-18)


### Performance Improvements

* **core:** reduce FFI allocation overhead ([#824](https://github.com/graydonpleasants/geo-polygonize/issues/824)) ([00a6651](https://github.com/graydonpleasants/geo-polygonize/commit/00a6651b2a4812dda92a7df811216e500a3ef4bc))

## [0.39.2](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.39.1...geo-polygonize-v0.39.2) (2026-07-18)


### Bug Fixes

* **core:** preserve GeoArrow metadata semantics ([#819](https://github.com/graydonpleasants/geo-polygonize/issues/819)) ([45a905c](https://github.com/graydonpleasants/geo-polygonize/commit/45a905cd287c5313f614ef307821edb04338683b))

## [0.39.1](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.39.0...geo-polygonize-v0.39.1) (2026-07-18)


### Bug Fixes

* **github:** restore JavaScript CI and npm publishing ([#816](https://github.com/graydonpleasants/geo-polygonize/issues/816)) ([315d56b](https://github.com/graydonpleasants/geo-polygonize/commit/315d56b8c77941ad33d4a57711bf0c2df3b4765d))

## [0.39.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.38.1...geo-polygonize-v0.39.0) (2026-07-18)


### Features

* **core:** add FlatGeobuf file polygonization ([#813](https://github.com/graydonpleasants/geo-polygonize/issues/813)) ([848dca5](https://github.com/graydonpleasants/geo-polygonize/commit/848dca59936da7675799cb027819d4547e26fc85))

## [0.38.1](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.38.0...geo-polygonize-v0.38.1) (2026-07-18)


### Bug Fixes

* **core:** decode GeoParquet before polygonizing ([#810](https://github.com/graydonpleasants/geo-polygonize/issues/810)) ([556e416](https://github.com/graydonpleasants/geo-polygonize/commit/556e4162531aea0fcf219ebbd4a4ef8d2bbea430))

## [0.38.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.37.8...geo-polygonize-v0.38.0) (2026-07-18)


### Features

* **core:** preserve coordinates through snap noding ([#803](https://github.com/graydonpleasants/geo-polygonize/issues/803)) ([fdeb78f](https://github.com/graydonpleasants/geo-polygonize/commit/fdeb78f6df8f9527325b3fbc80da8544e50565af))

## [0.37.8](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.37.7...geo-polygonize-v0.37.8) (2026-07-18)


### Performance Improvements

* **core:** remove per-batch SoA dispatch ([#801](https://github.com/graydonpleasants/geo-polygonize/issues/801)) ([1764ee3](https://github.com/graydonpleasants/geo-polygonize/commit/1764ee3f70157de4b345a0c382900a0f35523f01))

## [0.37.7](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.37.6...geo-polygonize-v0.37.7) (2026-07-18)


### Performance Improvements

* **core:** adapt point location for long rings ([#797](https://github.com/graydonpleasants/geo-polygonize/issues/797)) ([aa53fb0](https://github.com/graydonpleasants/geo-polygonize/commit/aa53fb080b5ead2535e7a9aa3689605def9d83f0))

## [0.37.6](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.37.5...geo-polygonize-v0.37.6) (2026-07-18)


### Performance Improvements

* **core:** benchmark fearless SIMD kernels ([#795](https://github.com/graydonpleasants/geo-polygonize/issues/795)) ([a8895e1](https://github.com/graydonpleasants/geo-polygonize/commit/a8895e1f52891012c04479faf3b47a5fcab28c18))

## [0.37.5](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.37.4...geo-polygonize-v0.37.5) (2026-07-17)


### Performance Improvements

* **core:** index pre-snap reference vertices ([#792](https://github.com/graydonpleasants/geo-polygonize/issues/792)) ([27dbd99](https://github.com/graydonpleasants/geo-polygonize/commit/27dbd99eb84f42dd9e75538c2b5b871469e7d48b))

## [0.37.4](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.37.3...geo-polygonize-v0.37.4) (2026-07-17)


### Performance Improvements

* **core:** use graph identity for touch checks ([#790](https://github.com/graydonpleasants/geo-polygonize/issues/790)) ([302355f](https://github.com/graydonpleasants/geo-polygonize/commit/302355f606226fd564e0cbc9d42adea9b0df65ee))

## [0.37.3](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.37.2...geo-polygonize-v0.37.3) (2026-07-17)


### Performance Improvements

* **core:** index repeated point location ([#787](https://github.com/graydonpleasants/geo-polygonize/issues/787)) ([12836a3](https://github.com/graydonpleasants/geo-polygonize/commit/12836a3518c603eb5f058860de8c0b6e1b76aaea))

## [0.37.2](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.37.1...geo-polygonize-v0.37.2) (2026-07-17)


### Performance Improvements

* **core:** benchmark adaptive point locators ([#785](https://github.com/graydonpleasants/geo-polygonize/issues/785)) ([5753b2f](https://github.com/graydonpleasants/geo-polygonize/commit/5753b2f3ca9fb6dbcf516124c12207a5ff65538c))

## [0.37.1](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.37.0...geo-polygonize-v0.37.1) (2026-07-17)


### Performance Improvements

* **core:** cache prepared ring metadata ([#782](https://github.com/graydonpleasants/geo-polygonize/issues/782)) ([f3ee972](https://github.com/graydonpleasants/geo-polygonize/commit/f3ee972ef87d20a69117c989e3cd0efad8087ed1))

## [0.37.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.36.2...geo-polygonize-v0.37.0) (2026-07-17)


### Features

* **core:** add noding audit profile and backend diagnostics ([#776](https://github.com/graydonpleasants/geo-polygonize/issues/776)) ([16457b0](https://github.com/graydonpleasants/geo-polygonize/commit/16457b082aafe627c6c730f17e2eb445ca811a75))

## [0.36.2](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.36.1...geo-polygonize-v0.36.2) (2026-07-08)


### Bug Fixes

* **core:** snap nearby endpoints during pre-snap ([#770](https://github.com/graydonpleasants/geo-polygonize/issues/770)) ([d4586b1](https://github.com/graydonpleasants/geo-polygonize/commit/d4586b10e7c4f4b5dae3cd83347a317e7f63ef58))

## [0.36.1](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.36.0...geo-polygonize-v0.36.1) (2026-07-08)


### Bug Fixes

* **github:** make release automation use PAT squash merges ([#764](https://github.com/graydonpleasants/geo-polygonize/issues/764)) ([e32b63e](https://github.com/graydonpleasants/geo-polygonize/commit/e32b63ec7657b6cbb19654e560eaa60ef161469f))

## [0.36.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.35.2...geo-polygonize-v0.36.0) (2026-07-08)


### Features

* **core:** add pre-snap parity option and lock CI checks ([beab8aa](https://github.com/graydonpleasants/geo-polygonize/commit/beab8aad4d23cfa7f00a0b3b9d0e91e91005d840))

## [0.35.2](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.35.1...geo-polygonize-v0.35.2) (2026-07-08)


### Bug Fixes

* **python:** include license files in sdist ([#753](https://github.com/graydonpleasants/geo-polygonize/issues/753)) ([a4fd019](https://github.com/graydonpleasants/geo-polygonize/commit/a4fd0197d0f9a245b4aae6de5ba8e9c340113b26))

## [0.35.1](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.35.0...geo-polygonize-v0.35.1) (2026-07-08)


### Bug Fixes

* **github:** make Python publish SBOM generation use pyproject metadata ([#749](https://github.com/graydonpleasants/geo-polygonize/issues/749)) ([c0032d6](https://github.com/graydonpleasants/geo-polygonize/commit/c0032d663fffcd0ef752aaa1e5460acadd49e35a))

## [0.35.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.34.0...geo-polygonize-v0.35.0) (2026-07-08)


### Features

* **python:** add import probe helper ([18866aa](https://github.com/graydonpleasants/geo-polygonize/commit/18866aadeb2e756d877a05f79c457deb1434f39e))


### Bug Fixes

* **wasm:** ship generated declarations ([e96cb7d](https://github.com/graydonpleasants/geo-polygonize/commit/e96cb7d84afb8c9fabe6d771a2aeff3ec61ad980))

## [0.34.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.33.0...geo-polygonize-v0.34.0) (2026-07-07)


### Features

* **core:** add CFB adoption hardening ([bbe187b](https://github.com/graydonpleasants/geo-polygonize/commit/bbe187b1625951df96f0d0ee4bba3268b511a09a))
* **core:** implement incremental stateful polygonizer ([#704](https://github.com/graydonpleasants/geo-polygonize/issues/704)) ([e88802b](https://github.com/graydonpleasants/geo-polygonize/commit/e88802b10a1a1a90de38c8b8b07da4b95b055971))


### Bug Fixes

* **playground:** correct vitepress build order and copy examples to public ([48c0a6f](https://github.com/graydonpleasants/geo-polygonize/commit/48c0a6fd81738483ad7735081556083e2a405794))
* **python:** upgrade pyo3 for advisory ([8f5b224](https://github.com/graydonpleasants/geo-polygonize/commit/8f5b224bf6101326c9cef1fa07ebeedbf2823cd1))


### Build System

* use full path for wasm-bindgen in build scripts ([d88e33b](https://github.com/graydonpleasants/geo-polygonize/commit/d88e33bc8363a7ce4749afdc4dfae1f24ba0af4c))

## [0.33.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.32.0...geo-polygonize-v0.33.0) (2026-03-30)


### Features

* **github:** revamp interactive documentation playground with SVG visualizer and comprehensive examples ([#694](https://github.com/graydonpleasants/geo-polygonize/issues/694)) ([1617023](https://github.com/graydonpleasants/geo-polygonize/commit/161702348367ea4edce471d59b3f1ac7051cd540))

## [0.32.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.31.0...geo-polygonize-v0.32.0) (2026-03-27)


### Features

* Add architecture-aware runtime SIMD dispatch for scalar, Wasm SIMD v128, x86_64 AVX2 ([e3ca33a](https://github.com/graydonpleasants/geo-polygonize/commit/e3ca33a4aa535f6d5d36481327e94c8228294a28))


### Bug Fixes

* disable multiversion macro on wasm32 to fix CI native dispatch failures ([1185832](https://github.com/graydonpleasants/geo-polygonize/commit/1185832f634e0b4ea5dd070b2d1fa81659204357))
* fix rustfmt warnings ([125a6fb](https://github.com/graydonpleasants/geo-polygonize/commit/125a6fb345c2cf9b2d4a8a22f4eb770ecad031c4))
* **github:** fix maintenance workflow infinite loop on automated pr merge ([#678](https://github.com/graydonpleasants/geo-polygonize/issues/678)) ([4d7c3bf](https://github.com/graydonpleasants/geo-polygonize/commit/4d7c3bfb705473734b3891f14932e2c12b20430c))
* remove wasm32+simd128 multiversion target ([af3b5b5](https://github.com/graydonpleasants/geo-polygonize/commit/af3b5b5f60696d725d5a59f88256e535f267a140))

## [0.31.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.30.2...geo-polygonize-v0.31.0) (2026-03-27)


### Features

* **core:** implement GeoParquet streaming IO across Rust, Wasm, Python ([#445](https://github.com/graydonpleasants/geo-polygonize/issues/445)) ([bd21255](https://github.com/graydonpleasants/geo-polygonize/commit/bd21255e7e330222fb1b5e17c62be7b97ad48635))


### Performance Improvements

* **core:** optimize canonicalize_ring with slice iterator ([64a1a90](https://github.com/graydonpleasants/geo-polygonize/commit/64a1a90a9b030b7b71fd068d4950b48e6a9536dd))
* **core:** Replace sort_by with sort_unstable_by and remove bounds checks ([#453](https://github.com/graydonpleasants/geo-polygonize/issues/453)) ([afea1d8](https://github.com/graydonpleasants/geo-polygonize/commit/afea1d81c9e2e27ee9fb20ae1d53d9c9706b3035))

## [0.30.2](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.30.1...geo-polygonize-v0.30.2) (2026-03-24)


### Performance Improvements

* optimize container counts iteration using iterators ([47d33d5](https://github.com/graydonpleasants/geo-polygonize/commit/47d33d57551f2aae0e2e0be6cb68521a1fe7eaf3))

## [0.30.1](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.30.0...geo-polygonize-v0.30.1) (2026-03-23)


### Bug Fixes

* set base path in docs and playground configs ([6e9e66f](https://github.com/graydonpleasants/geo-polygonize/commit/6e9e66f7a99686f437a70b8e68dfb6e1051894f5))

## [0.30.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.29.2...geo-polygonize-v0.30.0) (2026-03-23)


### Features

* implement O((N+K) log N) sweep-line noder with robust fallbacks ([7ff1e1d](https://github.com/graydonpleasants/geo-polygonize/commit/7ff1e1daeef7123d40c065f2c6f673c5b389c453))

## [0.29.2](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.29.1...geo-polygonize-v0.29.2) (2026-03-23)


### Bug Fixes

* ignore Vitepress dead link for playground ([84159bf](https://github.com/graydonpleasants/geo-polygonize/commit/84159bf741400c9761ec0bd01ddea88176139f16))

## [0.29.1](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.29.0...geo-polygonize-v0.29.1) (2026-03-23)


### Bug Fixes

* add license field to xtask Cargo.toml ([91f2aee](https://github.com/graydonpleasants/geo-polygonize/commit/91f2aee1fbec685d2a744aea8f596d594cd6d618))
* correctly run cargo fmt after fixing unstable iteration order ([e23b88b](https://github.com/graydonpleasants/geo-polygonize/commit/e23b88bfd63fc543455a8fad7e6824beccb16aff))
* stabilize docs generation output and add license to xtask ([8e1b9c0](https://github.com/graydonpleasants/geo-polygonize/commit/8e1b9c09ac68acf4f490954aad6366c6de17dd89))
* stabilize docs generation output with BTreeMap sorting ([4d3c245](https://github.com/graydonpleasants/geo-polygonize/commit/4d3c245455f7f13c680b57e70c6d6955e5f87bba))

## [0.29.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.28.0...geo-polygonize-v0.29.0) (2026-03-22)


### Features

* adaptive parallelism, fast math, and Wasm memory optimizations ([c46c370](https://github.com/graydonpleasants/geo-polygonize/commit/c46c370d23aca7a49060c520d8f1ecf79d48f5a8))
* Add PyO3 bindings and regenerate Cargo.lock ([a1e5602](https://github.com/graydonpleasants/geo-polygonize/commit/a1e560230e5bdc24d4c57dbdb696c76401c85c72))
* Add WASM SIMD support and publication enhancements ([015597e](https://github.com/graydonpleasants/geo-polygonize/commit/015597e58306a9d81523f89e44998fc4c24c0865))
* Add WASM SIMD support, feature detection, and publication pipeline ([32826e3](https://github.com/graydonpleasants/geo-polygonize/commit/32826e345b463473bff767f1a68d0a7bc7300b06))
* add webapp demo and wasm support ([f0548e7](https://github.com/graydonpleasants/geo-polygonize/commit/f0548e7e419962e882759c0bee2e428b43fc4ad7))
* Add zero-allocation area calculation methods to Polygon3D ([#155](https://github.com/graydonpleasants/geo-polygonize/issues/155)) ([c783c4a](https://github.com/graydonpleasants/geo-polygonize/commit/c783c4a85891dfb8d122c1925b837e7ca0fb3855))
* **api:** expose cross-binding polygonize_with_options ([44f9d43](https://github.com/graydonpleasants/geo-polygonize/commit/44f9d43b72ba88bcf8118a805474010c41fe9afc))
* architectural optimizations (ISR, SIMD, geo-index) ([95e58ca](https://github.com/graydonpleasants/geo-polygonize/commit/95e58ca3de5ce93e7af620ea97809534daea0fb9))
* architectural optimizations (ISR, SIMD) ([3071bd7](https://github.com/graydonpleasants/geo-polygonize/commit/3071bd7f72b52ec4e20f49ccbfe5c00c9f5ee198))
* architectural optimizations (ISR, SIMD) and robust benchmarking ([f4df36d](https://github.com/graydonpleasants/geo-polygonize/commit/f4df36dc601371578be053aeefe5d474cb7446c3))
* architectural optimizations (ISR, SIMD) with robust fallbacks ([2760a09](https://github.com/graydonpleasants/geo-polygonize/commit/2760a094020a3e913a947d6bec3aa01b18503f3c))
* architectural optimizations (ISR, SIMD) with robust fallbacks and clean dependencies ([23cd067](https://github.com/graydonpleasants/geo-polygonize/commit/23cd067e61c23021e8d71d2dc25f07a29b125369))
* architectural optimizations (wasm benchmark, geoarrow, talc) ([b818bce](https://github.com/graydonpleasants/geo-polygonize/commit/b818bce6896d901696a542bc3c01c5a948a80c65))
* Capture invalid rings in Polygonizer result ([987a1a6](https://github.com/graydonpleasants/geo-polygonize/commit/987a1a66be62d43b43c1c67c1c6fb312b12f995e))
* Capture invalid rings in Polygonizer result ([4a9f685](https://github.com/graydonpleasants/geo-polygonize/commit/4a9f6854179148389c9a633d3fe9de11f8f6025d))
* **core:** Add cargo-fuzz harnesses for roadmap fuzzing rollout ([b3e3646](https://github.com/graydonpleasants/geo-polygonize/commit/b3e3646bc6a6d58dd5201479ebbaf25ada2281d2))
* **core:** add cross-tile dedup via canonical ring hashing ([f736955](https://github.com/graydonpleasants/geo-polygonize/commit/f736955f910b21f2cba69a2aaf0e9ad7ec896593))
* **core:** add diagnostics and report mode scaffold ([0b1ca3a](https://github.com/graydonpleasants/geo-polygonize/commit/0b1ca3a390da6e1a1b8870dc3d75fadd2f1e459b))
* **core:** add per-polygon provenance with input_profile_id ([f7d68cc](https://github.com/graydonpleasants/geo-polygonize/commit/f7d68cc0be2012bf7108d905103486833b15b930))
* **core:** add provenance acceptance fixtures ([1ffe32c](https://github.com/graydonpleasants/geo-polygonize/commit/1ffe32c5bd10734881cef5c079f94258562a1edc))
* **core:** add SBOM generation, supply chain checks, and release provenance ([#353](https://github.com/graydonpleasants/geo-polygonize/issues/353)) ([6b1fa0d](https://github.com/graydonpleasants/geo-polygonize/commit/6b1fa0dc61a55b98346b5f487186ed9d02304773))
* **core:** extract containment forest and address roadmap updates ([6c3c48c](https://github.com/graydonpleasants/geo-polygonize/commit/6c3c48c3c929ac1ff11a7407eb4ebf03da58c573))
* **core:** fix formatting in cross-tile dedup ([e88165e](https://github.com/graydonpleasants/geo-polygonize/commit/e88165e3d72879191c9078db465bccec28fa43d5))
* **core:** implement canonical options schema ([#284](https://github.com/graydonpleasants/geo-polygonize/issues/284)) ([7fd8279](https://github.com/graydonpleasants/geo-polygonize/commit/7fd827979e19ba22c5b2b5e4fe8581e0578dcf48))
* **core:** implement deterministic output and canonical sorting ([58e16d5](https://github.com/graydonpleasants/geo-polygonize/commit/58e16d552001963b9ed539dadf4cdbce82d90964))
* **core:** implement deterministic output and canonical sorting ([356de05](https://github.com/graydonpleasants/geo-polygonize/commit/356de0580601edebbab9e81a541267d47eba754b))
* **core:** implement diagnostics collection and fix tests ([#266](https://github.com/graydonpleasants/geo-polygonize/issues/266)) ([44ef7f0](https://github.com/graydonpleasants/geo-polygonize/commit/44ef7f04fae06e16cdcb5ea8d92d5ae30bbfcbad))
* **core:** Implement hardened GeosCompat snap strategy ([#371](https://github.com/graydonpleasants/geo-polygonize/issues/371)) ([55fbcc9](https://github.com/graydonpleasants/geo-polygonize/commit/55fbcc9dd07a15fcfff9634b1c65466244a1db89))
* **core:** implement panic-safe boundaries and validation ([82f8cbb](https://github.com/graydonpleasants/geo-polygonize/commit/82f8cbb5d7fbd41e656f178ca9c15a2e889c481c))
* **core:** implement per-polygon provenance and profile passthrough ([845378b](https://github.com/graydonpleasants/geo-polygonize/commit/845378bb6c4ce66f9dcd7714b984f27f3824027b))
* **core:** implement touch policies and fix CI failures ([a97ac66](https://github.com/graydonpleasants/geo-polygonize/commit/a97ac66edd5491ac9286605c7fcfd8ec798f931b))
* **core:** implement typed Wasm and Python errors ([#273](https://github.com/graydonpleasants/geo-polygonize/issues/273)) ([f0e0b93](https://github.com/graydonpleasants/geo-polygonize/commit/f0e0b9351928a290dfe9c5a7154c834594d35402))
* **core:** Optimize geometry extraction by pre-allocating capacity using extend ([#172](https://github.com/graydonpleasants/geo-polygonize/issues/172)) ([922db67](https://github.com/graydonpleasants/geo-polygonize/commit/922db67c3540f6d15935579beaa82d1a7d503740))
* **core:** optimize noding float sorting & dedup logic ([#232](https://github.com/graydonpleasants/geo-polygonize/issues/232)) ([a458c15](https://github.com/graydonpleasants/geo-polygonize/commit/a458c154e3d28f2a684b0f296cd72f218b56c2b1))
* **core:** Optimize SimdRing capacity allocation ([6e320d5](https://github.com/graydonpleasants/geo-polygonize/commit/6e320d502d39099a829e695de93843d176756849))
* **core:** Optimize SimdRing capacity allocation ([8696685](https://github.com/graydonpleasants/geo-polygonize/commit/8696685ed1d02f83b35b568d19dee29ae3744db5))
* **core:** Optimize SimdRing capacity allocation ([71ae67a](https://github.com/graydonpleasants/geo-polygonize/commit/71ae67ac33a17212c807b3c12771250a1a039d82))
* **core:** panic-safe boundaries and validation ([444d7c9](https://github.com/graydonpleasants/geo-polygonize/commit/444d7c94999fc5551b543a7f08eb7325a99b3236))
* **core:** panic-safe boundaries and validation ([4c0b0c3](https://github.com/graydonpleasants/geo-polygonize/commit/4c0b0c336b52ea457289a332b9e42084535fb624))
* **core:** parallelize UniformGrid::new ([2e04f7e](https://github.com/graydonpleasants/geo-polygonize/commit/2e04f7ec89a1b76a6547bfadd0c757fa79c067f8))
* **core:** post benchmark results on PRs ([#176](https://github.com/graydonpleasants/geo-polygonize/issues/176)) ([b69e6a0](https://github.com/graydonpleasants/geo-polygonize/commit/b69e6a0418c2505d8f263e34f2849ba03222d107))
* **core:** Prototype optional advanced noder backend ([25e7eec](https://github.com/graydonpleasants/geo-polygonize/commit/25e7eec9b8e08af570c3f291838227b82d01608f))
* **core:** trigger release for bounding_rect optimization ([854e4ce](https://github.com/graydonpleasants/geo-polygonize/commit/854e4ce1162d8f414e531624629d700df4b42318))
* **deps:** add cargo fmt pre-commit hook to husky ([f09ea33](https://github.com/graydonpleasants/geo-polygonize/commit/f09ea330dfb501dd5538bb63e90321c1c6d5e4d4))
* **deps:** configure commitlint to strictly enforce conventional commits ([#203](https://github.com/graydonpleasants/geo-polygonize/issues/203)) ([23c6d13](https://github.com/graydonpleasants/geo-polygonize/commit/23c6d13020e4f45861657547e729dd06f93fd672))
* Document and verify `valid_edges` hoisting optimization in PlanarGraph ([a0797fe](https://github.com/graydonpleasants/geo-polygonize/commit/a0797fed4a0174e0c5c84acf4612149dddc9b3a9))
* expand differential testing suite with 6 new generators ([ee7f983](https://github.com/graydonpleasants/geo-polygonize/commit/ee7f983dcc4eeb53e8145e02b0265ed1eb83e15f))
* **github:** add automerge workflow for graydonpleasants ([1f79ec0](https://github.com/graydonpleasants/geo-polygonize/commit/1f79ec0e67b45f25f7288ec057864d501a6ad89b))
* **github:** Add manual `workflow_dispatch` release with version input to release-please workflow ([#231](https://github.com/graydonpleasants/geo-polygonize/issues/231)) ([a8177ce](https://github.com/graydonpleasants/geo-polygonize/commit/a8177ce4b5a6e0b4b343221e61839287099fe25a))
* implement spatial index backend abstraction and static packed index ([83d9cca](https://github.com/graydonpleasants/geo-polygonize/commit/83d9ccac012466f6d4c5e4ca4cbfe5e5e0dd0197))
* implement Uniform Grid noding and spatial sorting ([416a23a](https://github.com/graydonpleasants/geo-polygonize/commit/416a23a882830337a93739b4d2946375b4a0c5a7))
* Improve python bindings ergonomics and error handling ([#158](https://github.com/graydonpleasants/geo-polygonize/issues/158)) ([8d02115](https://github.com/graydonpleasants/geo-polygonize/commit/8d0211595eebfffea2a70ebc7b8871dbfa5b0559))
* Integrate FFI improvements and Python bindings ([25b210f](https://github.com/graydonpleasants/geo-polygonize/commit/25b210fa4c108d138fb26dd86d91f53477b257e3))
* **perf:** add iai-callgrind benchmarks and perf tracking workflow ([2af925f](https://github.com/graydonpleasants/geo-polygonize/commit/2af925ff51956c30c1124f4af4d0efccdf26c8a8))
* **perf:** add iai-callgrind benchmarks and perf tracking workflow ([3716a87](https://github.com/graydonpleasants/geo-polygonize/commit/3716a87d47a4ef3d387fec8df104c878455f09d3))
* **python:** add explain_mismatch to Python bindings ([e26293d](https://github.com/graydonpleasants/geo-polygonize/commit/e26293d4fc5f9a95813903efacb25ecf269c26b6))
* **python:** add safety checks for 3D and odd-length coords in python wrapper ([f6b0ec4](https://github.com/graydonpleasants/geo-polygonize/commit/f6b0ec4f8780e15eb3a50cd089eaf56727da5476))
* **python:** return SimplePolygon directly from PyO3 bindings ([#236](https://github.com/graydonpleasants/geo-polygonize/issues/236)) ([9977e68](https://github.com/graydonpleasants/geo-polygonize/commit/9977e6841c0157f1c5576235629b94b2cbe55c13))
* Refactor Wasm build pipeline and exports ([667fb1f](https://github.com/graydonpleasants/geo-polygonize/commit/667fb1ffbd180c6801e4f9e5184bdba78ff76df5))
* Setup fleet issue automation with stacked PR support ([b4855fe](https://github.com/graydonpleasants/geo-polygonize/commit/b4855fe94a3e78b15b5d7249a4370bef48fe27ab))
* Setup fleet issue automation with stacked PR support ([abf1b66](https://github.com/graydonpleasants/geo-polygonize/commit/abf1b66406746a6701fcdef25f2d8b7941b15c39))
* Setup fleet issue automation with stacked PR support ([a988eff](https://github.com/graydonpleasants/geo-polygonize/commit/a988effd089490b4b38c610567adab79fb409f89))
* Setup fleet issue automation with stacked PR support ([#148](https://github.com/graydonpleasants/geo-polygonize/issues/148)) ([0996161](https://github.com/graydonpleasants/geo-polygonize/commit/0996161ba610424b787c33627c8f00b94c83892a))
* stress test noding and verify benchmarks ([afdf207](https://github.com/graydonpleasants/geo-polygonize/commit/afdf207bb034147cbb9a05af1b903af52ba0ba52))
* **test:** add robust WASM malformed JSON tests ([abe0614](https://github.com/graydonpleasants/geo-polygonize/commit/abe061416e1896a6c8dbee2cc407cf5acace2d95))
* update Jules triggering workflows for task picking and roadmap analysis ([c49691a](https://github.com/graydonpleasants/geo-polygonize/commit/c49691a18de76d12bb158d05309d064ec8dfaa90))
* Wasm optimizations, benchmarks, and docs ([71d8e85](https://github.com/graydonpleasants/geo-polygonize/commit/71d8e8588f525f877bb5f749f7d8f9d1f2a294b7))
* **wasm:** add optional `line_ids` parameter and expose `flat_line_ids` ([2ea27d9](https://github.com/graydonpleasants/geo-polygonize/commit/2ea27d992cf7f8a13f378e761713ae3f114e469a))
* **wasm:** add optional `line_ids` parameter and expose `flat_line_ids` ([f50f999](https://github.com/graydonpleasants/geo-polygonize/commit/f50f999b21bc146efac2107c329d71787f4993c5))
* **wasm:** add Set and Map stringification to debug output in generated bindings ([4171203](https://github.com/graydonpleasants/geo-polygonize/commit/41712031fb297e1d1e60576f697b526a492e40fd))
* **wasm:** Expose explicit parity flags and document API ([#195](https://github.com/graydonpleasants/geo-polygonize/issues/195)) ([1271918](https://github.com/graydonpleasants/geo-polygonize/commit/12719182193a15b3cd97d55ed5cefdda736c45d7))
* **wasm:** integrate ts-rs for PolygonizerOptions bindings ([9f097d6](https://github.com/graydonpleasants/geo-polygonize/commit/9f097d6411df855b8ce5e5aa9a804ba0f45912b8))


### Bug Fixes

* add missing version to geo-polygonize-core dependency ([9f8d31d](https://github.com/graydonpleasants/geo-polygonize/commit/9f8d31d2639deef94bb8c95c0629526f435d4253))
* Apply cargo fmt to resolve CI failure in ffi.rs ([f951b18](https://github.com/graydonpleasants/geo-polygonize/commit/f951b18212228814b455f1f56938261efa430982))
* Apply cargo fmt to src/polygonizer.rs ([791d994](https://github.com/graydonpleasants/geo-polygonize/commit/791d994573b9a9fc45442ad37f3b598cc079f61f))
* **build:** revert wasm_bindgen upgrade causing build failures ([c96f4a2](https://github.com/graydonpleasants/geo-polygonize/commit/c96f4a21f70efffc81c536e44c8aa4ce329aa6eb))
* cargo fmt ([9996fad](https://github.com/graydonpleasants/geo-polygonize/commit/9996fad2fad65e2794d7843c969ef9333ba6f4ca))
* cargo fmt and ensure tests pass ([12f2aa8](https://github.com/graydonpleasants/geo-polygonize/commit/12f2aa8f69879095a82bbeb67b21a069241a2b62))
* cargo fmt in benches ([668edec](https://github.com/graydonpleasants/geo-polygonize/commit/668edecd3ecca2e0a4217c9e95481011051277be))
* **ci:** bump internal workspace dependencies during release-please sync ([#205](https://github.com/graydonpleasants/geo-polygonize/issues/205)) ([a26648c](https://github.com/graydonpleasants/geo-polygonize/commit/a26648c23d7f03137fa7c65dac3c265f58fb401d))
* **ci:** use time conditionally for wasm and fix python tests ([88d1242](https://github.com/graydonpleasants/geo-polygonize/commit/88d1242b6ecd1eb833d7f3d8adeea9e28ea3fad1))
* **core:** apply formatting to tiling.rs ([efcba02](https://github.com/graydonpleasants/geo-polygonize/commit/efcba025bd071843885a3347a86c4ca1025b12ec))
* **core:** apply ring rotation independently and fully order dangles ([202aa3a](https://github.com/graydonpleasants/geo-polygonize/commit/202aa3ac93a53989852e2772820bf38027384a17))
* **core:** enforce strict golden fixture assertions ([f6fba68](https://github.com/graydonpleasants/geo-polygonize/commit/f6fba6827ed3db8c95406e5102b4f0904c272576))
* **core:** enforce strict golden fixture assertions ([#272](https://github.com/graydonpleasants/geo-polygonize/issues/272)) ([fee9cd1](https://github.com/graydonpleasants/geo-polygonize/commit/fee9cd1c6f749fffbcaa1a8133e8f44fb26644bb))
* **core:** format files and correct touch policies implementation ([65479a5](https://github.com/graydonpleasants/geo-polygonize/commit/65479a5958f61b99c351c54f5c38f7d452e6aff8))
* **core:** improve edge sharing detection logic and add tests ([e8fc295](https://github.com/graydonpleasants/geo-polygonize/commit/e8fc29546705036f21fce2a07c4ef54fabacc9d0))
* **core:** improve edge sharing detection logic and add tests ([24973ad](https://github.com/graydonpleasants/geo-polygonize/commit/24973ad25f8f2919604e4dc44e5673fb803bb310))
* **core:** prevent potential panic by using safe zip iteration ([#186](https://github.com/graydonpleasants/geo-polygonize/issues/186)) ([29ce3ad](https://github.com/graydonpleasants/geo-polygonize/commit/29ce3ad9c6051f905e32629d60ea704d8d566783))
* **core:** remove debug print in polygonizer.rs ([6c77240](https://github.com/graydonpleasants/geo-polygonize/commit/6c772401865aa1d230e1c7fb1d1c14dfb07bc508))
* **core:** resolve clippy collapsible_if and thread_local lints ([635af15](https://github.com/graydonpleasants/geo-polygonize/commit/635af15f499472824999e2ff36d9ee1a4ae98fb1))
* **core:** tighten determinism for dangles and ring rotation ([#270](https://github.com/graydonpleasants/geo-polygonize/issues/270)) ([1fcb3d8](https://github.com/graydonpleasants/geo-polygonize/commit/1fcb3d8476f0ecae1b9918c9141e3982441d2bb8))
* **github:** cleanup repo ([7a31c35](https://github.com/graydonpleasants/geo-polygonize/commit/7a31c353243cbc8587b5d5ef312e04a2097e041e))
* **github:** cleaunup ([9c1fa25](https://github.com/graydonpleasants/geo-polygonize/commit/9c1fa2592cb6769b4e713ebfd0bf29342bfb3cc4))
* **github:** fix jules automation mode in fleet scripts ([#392](https://github.com/graydonpleasants/geo-polygonize/issues/392)) ([ab2e31d](https://github.com/graydonpleasants/geo-polygonize/commit/ab2e31df470df1e6b0df0ec0f114cde986ec4d63))
* **github:** restore lost cron workflow changes ([#389](https://github.com/graydonpleasants/geo-polygonize/issues/389)) ([d4cb13e](https://github.com/graydonpleasants/geo-polygonize/commit/d4cb13ebeaa1b72ecdef09f41738a9f5ae09470f))
* **github:** support release-please component tags in publish workflows ([#207](https://github.com/graydonpleasants/geo-polygonize/issues/207)) ([b59cccd](https://github.com/graydonpleasants/geo-polygonize/commit/b59cccd116c8225257d2bacdc1f78c6dee8fe93a))
* **github:** track version updates for python and rust crates ([#192](https://github.com/graydonpleasants/geo-polygonize/issues/192)) ([35e5684](https://github.com/graydonpleasants/geo-polygonize/commit/35e56849174f1efd8e8a6f52bf2fb87e0049af53))
* **github:** track version updates for python and rust crates with manifest config ([#193](https://github.com/graydonpleasants/geo-polygonize/issues/193)) ([bf5a7a0](https://github.com/graydonpleasants/geo-polygonize/commit/bf5a7a0f1518224446960decb46831d759ecd52b))
* **github:** upgrade npm before trusted npm publish ([bfab447](https://github.com/graydonpleasants/geo-polygonize/commit/bfab447544b64e06e99d09dcc432d7600b67ba54))
* **github:** use trusted publishing for npm ([#181](https://github.com/graydonpleasants/geo-polygonize/issues/181)) ([57a47f3](https://github.com/graydonpleasants/geo-polygonize/commit/57a47f3a84ae9846d2dcc4f906643f26759f1513))
* Hyper-optimize Core Noding & Parallelism Framework ([4f2c557](https://github.com/graydonpleasants/geo-polygonize/commit/4f2c5573207f22a0330a9849a6679c857e29ee4f))
* Hyper-optimize Core Noding & Parallelism Framework ([53a7f6c](https://github.com/graydonpleasants/geo-polygonize/commit/53a7f6c15fe795a45c80cc92a3b849dccc9b3857))
* implement typed binding errors for python and wasm properly ([61913bc](https://github.com/graydonpleasants/geo-polygonize/commit/61913bc3b43db021e67a2bb710762ed368bfe7ca))
* implement typed binding errors for python and wasm properly ([8ff1fe7](https://github.com/graydonpleasants/geo-polygonize/commit/8ff1fe799d43dbc768b9bc6904dfe5edcfd2fdf6))
* Improve Python error handling by checking CPolygonStatus ([7531506](https://github.com/graydonpleasants/geo-polygonize/commit/7531506f2ed5dc1053416515b88b3ff6e3818c1d))
* **python:** explicitly codesign macOS wheels to prevent EXC_BAD_ACCESS ([#190](https://github.com/graydonpleasants/geo-polygonize/issues/190)) ([c6129aa](https://github.com/graydonpleasants/geo-polygonize/commit/c6129aa651428fd732e7dcb477272075231f3aad))
* **python:** remove invalid `--release` argument from maturin publish command ([#179](https://github.com/graydonpleasants/geo-polygonize/issues/179)) ([b147494](https://github.com/graydonpleasants/geo-polygonize/commit/b147494649fd0a24e79061b9e747539ba8e0d11c))
* **python:** rename python package to geo-polygonize-py to avoid PyPI name collision ([#180](https://github.com/graydonpleasants/geo-polygonize/issues/180)) ([a24e862](https://github.com/graydonpleasants/geo-polygonize/commit/a24e86282117cdfc874e470c1ee7134b461d7718))
* Resolve active_cells unwrap panic ([d62f3bc](https://github.com/graydonpleasants/geo-polygonize/commit/d62f3bcb79a3d640facafa1a42ac9b7bd83c0970))
* resolve clippy warnings in noding/snap.rs ([03e41e2](https://github.com/graydonpleasants/geo-polygonize/commit/03e41e2ebea101de93c1d9ac6f0b6aa623416f0e))
* Resolve unwrap in `not(feature = "parallel")` path ([d70f234](https://github.com/graydonpleasants/geo-polygonize/commit/d70f234395fed98b0303e95ce36dd1285c9842a4))
* Run cargo fmt to pass CI ([b6852fe](https://github.com/graydonpleasants/geo-polygonize/commit/b6852fe7ca2ae444101b8722f68fdb94f1f1b20f))
* rustfmt formatting on spatial index backend code ([3722503](https://github.com/graydonpleasants/geo-polygonize/commit/37225035e1f5e1d609170835e56e8f06dabe063d))
* **security:** prevent panic in hole assignment ([9e27230](https://github.com/graydonpleasants/geo-polygonize/commit/9e272307df8d8ddf5bf0d120174eb7d1d0cc2dbe))
* use custom token for release-please to allow PR creation ([c11acb7](https://github.com/graydonpleasants/geo-polygonize/commit/c11acb7ed628313df18a770910797499fa9caa46))
* **wasm:** add repository url for npm provenance ([46736de](https://github.com/graydonpleasants/geo-polygonize/commit/46736deb7a73fe9f708aecfa8f4de944b9de6d33))
* **wasm:** update wasm-bindgen-rayon import resolution to use import.meta.resolve ([8dd29c5](https://github.com/graydonpleasants/geo-polygonize/commit/8dd29c5f9e1eacff53e5ac6e816aa702f8d7b546))


### Performance Improvements

* **core:** ⚡ Bolt: cache ring areas for sorting ([e27fe69](https://github.com/graydonpleasants/geo-polygonize/commit/e27fe698656b4b269c243efcb685d8c9d19c6f37))
* **core:** ⚡ Bolt: cache ring areas for sorting (fix fmt) ([8dc1ffb](https://github.com/graydonpleasants/geo-polygonize/commit/8dc1ffbc578df33e13d005cfaf77850f55effd69))
* **core:** address roadmap and version bump nitpicks ([#306](https://github.com/graydonpleasants/geo-polygonize/issues/306)) ([8c38a67](https://github.com/graydonpleasants/geo-polygonize/commit/8c38a672f69f9c2fd1c403287673913de55ad8d5))
* **core:** avoid cloning geometries in tiling logic ([57eef63](https://github.com/graydonpleasants/geo-polygonize/commit/57eef63a550c19e403ac8ec65bbc29c19350248d))
* **core:** Avoid cloning geometries inside TiledPolygonizer ([fdf513d](https://github.com/graydonpleasants/geo-polygonize/commit/fdf513d180e8c8a246963a78fc6cad09e6ca621e))
* **core:** Avoid cloning geometries inside TiledPolygonizer ([5f33624](https://github.com/graydonpleasants/geo-polygonize/commit/5f336249fb6048ed1ee12e9755e1322af05a7a7f))
* **core:** eager initialization of SimdRing objects over OnceLock ([628bfdd](https://github.com/graydonpleasants/geo-polygonize/commit/628bfdd7400c3bcf9be57c6f49eacc28b8ed0cad))
* **core:** Eliminate intermediate polygon allocations ([#204](https://github.com/graydonpleasants/geo-polygonize/issues/204)) ([5329162](https://github.com/graydonpleasants/geo-polygonize/commit/5329162172980c3f6bc154c3ef7a9986b7fe2838))
* **core:** eliminate unnecessary clone of hole coordinates in assignment ([08a51ba](https://github.com/graydonpleasants/geo-polygonize/commit/08a51ba41543981fd870f12ccc4cb5109c0bda5a))
* **core:** optimize area and centroid loop bounds-checking ([6e23dcc](https://github.com/graydonpleasants/geo-polygonize/commit/6e23dccda8c09cd455dd827d46c44f559991065d))
* **core:** optimize canonical sorts using Schwartzian Transform ([cda3e20](https://github.com/graydonpleasants/geo-polygonize/commit/cda3e203b33d6cb5831060639ada8f909530688f))
* **core:** optimize containment checks with exterior area ([5c34f5f](https://github.com/graydonpleasants/geo-polygonize/commit/5c34f5fbd9c9f2a6b338541612540f4328706273))
* **core:** optimize extract_segments by using iterative SmallVec stack and pre-allocating segment vectors ([6a8447b](https://github.com/graydonpleasants/geo-polygonize/commit/6a8447b12d4907d751f6b0162b039ec96e6e144d))
* **core:** optimize guaranteed interior probe loop ([#326](https://github.com/graydonpleasants/geo-polygonize/issues/326)) ([63da01c](https://github.com/graydonpleasants/geo-polygonize/commit/63da01cf50c0ae868658ab254661d984085b4645))
* **core:** optimize invalid ring sorting and interior probe logic ([ffaee4b](https://github.com/graydonpleasants/geo-polygonize/commit/ffaee4bc609bca1af42d73c77e3829efcc1ee526))
* **core:** optimize lexicographic min vertex search in tiling ([f42d529](https://github.com/graydonpleasants/geo-polygonize/commit/f42d529086b3677a1ba9cbe86d749a19c3172481))
* **core:** optimize polygon construction loop by avoiding clones ([#187](https://github.com/graydonpleasants/geo-polygonize/issues/187)) ([8ae1e7d](https://github.com/graydonpleasants/geo-polygonize/commit/8ae1e7d0d347b7938538c055e321bb3e2c97d7e6))
* **core:** optimize rings_share_edge evaluation and simd_shells filtering ([db8d3c1](https://github.com/graydonpleasants/geo-polygonize/commit/db8d3c1e75405fbe8e672a70f57e503e66789379))
* **core:** optimize SimdRing array initialization ([d234af5](https://github.com/graydonpleasants/geo-polygonize/commit/d234af56b6bdd48b1eae3e0c5ea19ade89cc92f3))
* **core:** Use `.windows(2)` iterators over index-based loops in rings_share_edge ([1be2ea3](https://github.com/graydonpleasants/geo-polygonize/commit/1be2ea3c0564da28dc5e475fd958e1c78e9e8dba))
* **core:** Use exterior area for canonical sort ([0e3888f](https://github.com/graydonpleasants/geo-polygonize/commit/0e3888f8bdc11ede51401f6cba3d69181353f79d))
* **core:** Use windows iterator in add_line_string ([81fe8ca](https://github.com/graydonpleasants/geo-polygonize/commit/81fe8cae515e3bc69fb89404db70e6ca82af47af))
* optimize hot loops by reusing vectors and avoiding clones ([9f600d8](https://github.com/graydonpleasants/geo-polygonize/commit/9f600d82e98967452655b640191bfc9ce97e7b4f))
* Optimize Polygonizer R-Tree allocation ([6322d6d](https://github.com/graydonpleasants/geo-polygonize/commit/6322d6dca95734c18eceafcdf4a5aa7ccee90347))
* Optimize SnapNoder by removing splits instead of cloning ([494469a](https://github.com/graydonpleasants/geo-polygonize/commit/494469a4a3b6b389f1a27d5eb73352d681cc274d))
* **python:** optimize result parsing in cffi wrapper ([b65722f](https://github.com/graydonpleasants/geo-polygonize/commit/b65722f8f184e6d319ec1b77a9a007b62c164d70))


### Build System

* **core:** fix formatting in extract_bench.rs ([85bf807](https://github.com/graydonpleasants/geo-polygonize/commit/85bf80760d7d0b7679d3a270bdc0087cf7800917))
* **github:** unhide build commits in release-please config ([#198](https://github.com/graydonpleasants/geo-polygonize/issues/198)) ([913fb9c](https://github.com/graydonpleasants/geo-polygonize/commit/913fb9c3ed932dac1c80f0da488d76201d3f753b))
* update geo-polygonize-core version in wasm Cargo.toml ([#196](https://github.com/graydonpleasants/geo-polygonize/issues/196)) ([0322e06](https://github.com/graydonpleasants/geo-polygonize/commit/0322e066c74a7f707196b3e2155c539d13b62f2d))
* **wasm:** update geo-polygonize-core version in Cargo.toml ([#197](https://github.com/graydonpleasants/geo-polygonize/issues/197)) ([d2a47c7](https://github.com/graydonpleasants/geo-polygonize/commit/d2a47c78034bed0d15b7649f42189118ad034f05))
* **wasm:** use trusted publishing for npm ([#183](https://github.com/graydonpleasants/geo-polygonize/issues/183)) ([5d5d0f1](https://github.com/graydonpleasants/geo-polygonize/commit/5d5d0f15a3addf5cf50189c689cd678d43bc1be0))

## [0.28.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.27.0...geo-polygonize-v0.28.0) (2026-03-22)


### Features

* **core:** add provenance acceptance fixtures ([1ffe32c](https://github.com/graydonpleasants/geo-polygonize/commit/1ffe32c5bd10734881cef5c079f94258562a1edc))


### Bug Fixes

* **github:** fix jules automation mode in fleet scripts ([#392](https://github.com/graydonpleasants/geo-polygonize/issues/392)) ([ab2e31d](https://github.com/graydonpleasants/geo-polygonize/commit/ab2e31df470df1e6b0df0ec0f114cde986ec4d63))

## [0.27.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.26.0...geo-polygonize-v0.27.0) (2026-03-22)


### Features

* **perf:** add iai-callgrind benchmarks and perf tracking workflow ([2af925f](https://github.com/graydonpleasants/geo-polygonize/commit/2af925ff51956c30c1124f4af4d0efccdf26c8a8))
* **perf:** add iai-callgrind benchmarks and perf tracking workflow ([3716a87](https://github.com/graydonpleasants/geo-polygonize/commit/3716a87d47a4ef3d387fec8df104c878455f09d3))


### Bug Fixes

* **build:** revert wasm_bindgen upgrade causing build failures ([c96f4a2](https://github.com/graydonpleasants/geo-polygonize/commit/c96f4a21f70efffc81c536e44c8aa4ce329aa6eb))
* **github:** restore lost cron workflow changes ([#389](https://github.com/graydonpleasants/geo-polygonize/issues/389)) ([d4cb13e](https://github.com/graydonpleasants/geo-polygonize/commit/d4cb13ebeaa1b72ecdef09f41738a9f5ae09470f))

## [0.26.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.25.0...geo-polygonize-v0.26.0) (2026-03-22)


### Features

* **core:** Implement hardened GeosCompat snap strategy ([#371](https://github.com/graydonpleasants/geo-polygonize/issues/371)) ([55fbcc9](https://github.com/graydonpleasants/geo-polygonize/commit/55fbcc9dd07a15fcfff9634b1c65466244a1db89))
* **python:** add explain_mismatch to Python bindings ([e26293d](https://github.com/graydonpleasants/geo-polygonize/commit/e26293d4fc5f9a95813903efacb25ecf269c26b6))


### Bug Fixes

* **wasm:** update wasm-bindgen-rayon import resolution to use import.meta.resolve ([8dd29c5](https://github.com/graydonpleasants/geo-polygonize/commit/8dd29c5f9e1eacff53e5ac6e816aa702f8d7b546))

## [0.25.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.24.0...geo-polygonize-v0.25.0) (2026-03-22)


### Features

* **core:** Implement hardened GeosCompat snap strategy ([#371](https://github.com/graydonpleasants/geo-polygonize/issues/371)) ([55fbcc9](https://github.com/graydonpleasants/geo-polygonize/commit/55fbcc9dd07a15fcfff9634b1c65466244a1db89))
* **wasm:** add Set and Map stringification to debug output in generated bindings ([4171203](https://github.com/graydonpleasants/geo-polygonize/commit/41712031fb297e1d1e60576f697b526a492e40fd))


### Bug Fixes

* **wasm:** update wasm-bindgen-rayon import resolution to use import.meta.resolve ([8dd29c5](https://github.com/graydonpleasants/geo-polygonize/commit/8dd29c5f9e1eacff53e5ac6e816aa702f8d7b546))

## [0.24.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.23.0...geo-polygonize-v0.24.0) (2026-03-22)


### Features

* **core:** add SBOM generation, supply chain checks, and release provenance ([#353](https://github.com/graydonpleasants/geo-polygonize/issues/353)) ([6b1fa0d](https://github.com/graydonpleasants/geo-polygonize/commit/6b1fa0dc61a55b98346b5f487186ed9d02304773))
* **core:** Implement hardened GeosCompat snap strategy ([#371](https://github.com/graydonpleasants/geo-polygonize/issues/371)) ([55fbcc9](https://github.com/graydonpleasants/geo-polygonize/commit/55fbcc9dd07a15fcfff9634b1c65466244a1db89))
* **wasm:** add Set and Map stringification to debug output in generated bindings ([4171203](https://github.com/graydonpleasants/geo-polygonize/commit/41712031fb297e1d1e60576f697b526a492e40fd))


### Bug Fixes

* **core:** apply formatting to tiling.rs ([efcba02](https://github.com/graydonpleasants/geo-polygonize/commit/efcba025bd071843885a3347a86c4ca1025b12ec))


### Performance Improvements

* **core:** optimize lexicographic min vertex search in tiling ([f42d529](https://github.com/graydonpleasants/geo-polygonize/commit/f42d529086b3677a1ba9cbe86d749a19c3172481))

## [0.23.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.22.1...geo-polygonize-v0.23.0) (2026-03-21)


### Features

* **core:** add SBOM generation, supply chain checks, and release provenance ([#353](https://github.com/graydonpleasants/geo-polygonize/issues/353)) ([6b1fa0d](https://github.com/graydonpleasants/geo-polygonize/commit/6b1fa0dc61a55b98346b5f487186ed9d02304773))
* update Jules triggering workflows for task picking and roadmap analysis ([c49691a](https://github.com/graydonpleasants/geo-polygonize/commit/c49691a18de76d12bb158d05309d064ec8dfaa90))


### Bug Fixes

* **core:** apply formatting to tiling.rs ([efcba02](https://github.com/graydonpleasants/geo-polygonize/commit/efcba025bd071843885a3347a86c4ca1025b12ec))


### Performance Improvements

* **core:** optimize lexicographic min vertex search in tiling ([f42d529](https://github.com/graydonpleasants/geo-polygonize/commit/f42d529086b3677a1ba9cbe86d749a19c3172481))

## [0.22.1](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.22.0...geo-polygonize-v0.22.1) (2026-03-21)


### Performance Improvements

* **core:** optimize canonical sorts using Schwartzian Transform ([cda3e20](https://github.com/graydonpleasants/geo-polygonize/commit/cda3e203b33d6cb5831060639ada8f909530688f))

## [0.22.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.21.0...geo-polygonize-v0.22.0) (2026-03-21)


### Features

* **core:** Prototype optional advanced noder backend ([25e7eec](https://github.com/graydonpleasants/geo-polygonize/commit/25e7eec9b8e08af570c3f291838227b82d01608f))


### Bug Fixes

* Run cargo fmt to pass CI ([b6852fe](https://github.com/graydonpleasants/geo-polygonize/commit/b6852fe7ca2ae444101b8722f68fdb94f1f1b20f))

## [0.21.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.20.0...geo-polygonize-v0.21.0) (2026-03-21)


### Features

* **core:** Add cargo-fuzz harnesses for roadmap fuzzing rollout ([b3e3646](https://github.com/graydonpleasants/geo-polygonize/commit/b3e3646bc6a6d58dd5201479ebbaf25ada2281d2))

## [0.20.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.19.0...geo-polygonize-v0.20.0) (2026-03-21)


### Features

* **core:** parallelize UniformGrid::new ([2e04f7e](https://github.com/graydonpleasants/geo-polygonize/commit/2e04f7ec89a1b76a6547bfadd0c757fa79c067f8))

## [0.19.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.18.1...geo-polygonize-v0.19.0) (2026-03-21)


### Features

* implement spatial index backend abstraction and static packed index ([83d9cca](https://github.com/graydonpleasants/geo-polygonize/commit/83d9ccac012466f6d4c5e4ca4cbfe5e5e0dd0197))


### Bug Fixes

* rustfmt formatting on spatial index backend code ([3722503](https://github.com/graydonpleasants/geo-polygonize/commit/37225035e1f5e1d609170835e56e8f06dabe063d))

## [0.18.1](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.18.0...geo-polygonize-v0.18.1) (2026-03-20)


### Performance Improvements

* **core:** optimize invalid ring sorting and interior probe logic ([ffaee4b](https://github.com/graydonpleasants/geo-polygonize/commit/ffaee4bc609bca1af42d73c77e3829efcc1ee526))

## [0.18.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.17.4...geo-polygonize-v0.18.0) (2026-03-20)


### Features

* **core:** add cross-tile dedup via canonical ring hashing ([f736955](https://github.com/graydonpleasants/geo-polygonize/commit/f736955f910b21f2cba69a2aaf0e9ad7ec896593))
* **core:** fix formatting in cross-tile dedup ([e88165e](https://github.com/graydonpleasants/geo-polygonize/commit/e88165e3d72879191c9078db465bccec28fa43d5))

## [0.17.4](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.17.3...geo-polygonize-v0.17.4) (2026-03-19)


### Performance Improvements

* **core:** optimize guaranteed interior probe loop ([#326](https://github.com/graydonpleasants/geo-polygonize/issues/326)) ([63da01c](https://github.com/graydonpleasants/geo-polygonize/commit/63da01cf50c0ae868658ab254661d984085b4645))

## [0.17.3](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.17.2...geo-polygonize-v0.17.3) (2026-03-19)


### Bug Fixes

* implement typed binding errors for python and wasm properly ([61913bc](https://github.com/graydonpleasants/geo-polygonize/commit/61913bc3b43db021e67a2bb710762ed368bfe7ca))
* implement typed binding errors for python and wasm properly ([8ff1fe7](https://github.com/graydonpleasants/geo-polygonize/commit/8ff1fe799d43dbc768b9bc6904dfe5edcfd2fdf6))

## [0.17.2](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.17.1...geo-polygonize-v0.17.2) (2026-03-19)


### Bug Fixes

* **core:** improve edge sharing detection logic and add tests ([e8fc295](https://github.com/graydonpleasants/geo-polygonize/commit/e8fc29546705036f21fce2a07c4ef54fabacc9d0))
* **core:** improve edge sharing detection logic and add tests ([24973ad](https://github.com/graydonpleasants/geo-polygonize/commit/24973ad25f8f2919604e4dc44e5673fb803bb310))


### Performance Improvements

* **core:** ⚡ Bolt: cache ring areas for sorting ([e27fe69](https://github.com/graydonpleasants/geo-polygonize/commit/e27fe698656b4b269c243efcb685d8c9d19c6f37))
* **core:** ⚡ Bolt: cache ring areas for sorting (fix fmt) ([8dc1ffb](https://github.com/graydonpleasants/geo-polygonize/commit/8dc1ffbc578df33e13d005cfaf77850f55effd69))

## [0.17.1](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.17.0...geo-polygonize-v0.17.1) (2026-03-17)


### Performance Improvements

* **core:** Use exterior area for canonical sort ([0e3888f](https://github.com/graydonpleasants/geo-polygonize/commit/0e3888f8bdc11ede51401f6cba3d69181353f79d))

## [0.17.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.16.0...geo-polygonize-v0.17.0) (2026-03-17)


### Features

* **wasm:** integrate ts-rs for PolygonizerOptions bindings ([9f097d6](https://github.com/graydonpleasants/geo-polygonize/commit/9f097d6411df855b8ce5e5aa9a804ba0f45912b8))

## [0.16.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.15.0...geo-polygonize-v0.16.0) (2026-03-16)


### Features

* **core:** extract containment forest and address roadmap updates ([6c3c48c](https://github.com/graydonpleasants/geo-polygonize/commit/6c3c48c3c929ac1ff11a7407eb4ebf03da58c573))
* **core:** implement touch policies and fix CI failures ([a97ac66](https://github.com/graydonpleasants/geo-polygonize/commit/a97ac66edd5491ac9286605c7fcfd8ec798f931b))


### Bug Fixes

* **core:** format files and correct touch policies implementation ([65479a5](https://github.com/graydonpleasants/geo-polygonize/commit/65479a5958f61b99c351c54f5c38f7d452e6aff8))

## [0.15.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.14.1...geo-polygonize-v0.15.0) (2026-03-15)


### Features

* **deps:** add cargo fmt pre-commit hook to husky ([f09ea33](https://github.com/graydonpleasants/geo-polygonize/commit/f09ea330dfb501dd5538bb63e90321c1c6d5e4d4))


### Performance Improvements

* **core:** address roadmap and version bump nitpicks ([#306](https://github.com/graydonpleasants/geo-polygonize/issues/306)) ([8c38a67](https://github.com/graydonpleasants/geo-polygonize/commit/8c38a672f69f9c2fd1c403287673913de55ad8d5))

## [0.14.1](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.14.0...geo-polygonize-v0.14.1) (2026-03-15)


### Performance Improvements

* **core:** Use windows iterator in add_line_string ([81fe8ca](https://github.com/graydonpleasants/geo-polygonize/commit/81fe8cae515e3bc69fb89404db70e6ca82af47af))

## [0.14.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.13.0...geo-polygonize-v0.14.0) (2026-03-15)


### Features

* **core:** implement per-polygon provenance and profile passthrough ([845378b](https://github.com/graydonpleasants/geo-polygonize/commit/845378bb6c4ce66f9dcd7714b984f27f3824027b))

## [0.13.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.12.0...geo-polygonize-v0.13.0) (2026-03-13)


### Features

* **wasm:** add optional `line_ids` parameter and expose `flat_line_ids` ([2ea27d9](https://github.com/graydonpleasants/geo-polygonize/commit/2ea27d992cf7f8a13f378e761713ae3f114e469a))
* **wasm:** add optional `line_ids` parameter and expose `flat_line_ids` ([f50f999](https://github.com/graydonpleasants/geo-polygonize/commit/f50f999b21bc146efac2107c329d71787f4993c5))


### Bug Fixes

* **github:** cleanup repo ([7a31c35](https://github.com/graydonpleasants/geo-polygonize/commit/7a31c353243cbc8587b5d5ef312e04a2097e041e))


### Performance Improvements

* **core:** optimize SimdRing array initialization ([d234af5](https://github.com/graydonpleasants/geo-polygonize/commit/d234af56b6bdd48b1eae3e0c5ea19ade89cc92f3))

## [0.12.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.11.0...geo-polygonize-v0.12.0) (2026-03-13)


### Features

* **core:** add per-polygon provenance with input_profile_id ([f7d68cc](https://github.com/graydonpleasants/geo-polygonize/commit/f7d68cc0be2012bf7108d905103486833b15b930))


### Bug Fixes

* cargo fmt and ensure tests pass ([12f2aa8](https://github.com/graydonpleasants/geo-polygonize/commit/12f2aa8f69879095a82bbeb67b21a069241a2b62))

## [0.11.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.10.0...geo-polygonize-v0.11.0) (2026-03-12)


### Features

* **api:** expose cross-binding polygonize_with_options ([44f9d43](https://github.com/graydonpleasants/geo-polygonize/commit/44f9d43b72ba88bcf8118a805474010c41fe9afc))

## [0.10.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.9.0...geo-polygonize-v0.10.0) (2026-03-12)


### Features

* **core:** implement canonical options schema ([#284](https://github.com/graydonpleasants/geo-polygonize/issues/284)) ([7fd8279](https://github.com/graydonpleasants/geo-polygonize/commit/7fd827979e19ba22c5b2b5e4fe8581e0578dcf48))


### Performance Improvements

* **core:** optimize containment checks with exterior area ([5c34f5f](https://github.com/graydonpleasants/geo-polygonize/commit/5c34f5fbd9c9f2a6b338541612540f4328706273))
* **core:** Use `.windows(2)` iterators over index-based loops in rings_share_edge ([1be2ea3](https://github.com/graydonpleasants/geo-polygonize/commit/1be2ea3c0564da28dc5e475fd958e1c78e9e8dba))
* **python:** optimize result parsing in cffi wrapper ([b65722f](https://github.com/graydonpleasants/geo-polygonize/commit/b65722f8f184e6d319ec1b77a9a007b62c164d70))

## [0.9.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.8.0...geo-polygonize-v0.9.0) (2026-03-11)


### Features

* **core:** add diagnostics and report mode scaffold ([0b1ca3a](https://github.com/graydonpleasants/geo-polygonize/commit/0b1ca3a390da6e1a1b8870dc3d75fadd2f1e459b))
* **core:** implement panic-safe boundaries and validation ([82f8cbb](https://github.com/graydonpleasants/geo-polygonize/commit/82f8cbb5d7fbd41e656f178ca9c15a2e889c481c))
* **core:** implement typed Wasm and Python errors ([#273](https://github.com/graydonpleasants/geo-polygonize/issues/273)) ([f0e0b93](https://github.com/graydonpleasants/geo-polygonize/commit/f0e0b9351928a290dfe9c5a7154c834594d35402))
* **core:** Optimize SimdRing capacity allocation ([6e320d5](https://github.com/graydonpleasants/geo-polygonize/commit/6e320d502d39099a829e695de93843d176756849))
* **core:** Optimize SimdRing capacity allocation ([8696685](https://github.com/graydonpleasants/geo-polygonize/commit/8696685ed1d02f83b35b568d19dee29ae3744db5))
* **core:** Optimize SimdRing capacity allocation ([71ae67a](https://github.com/graydonpleasants/geo-polygonize/commit/71ae67ac33a17212c807b3c12771250a1a039d82))
* **core:** panic-safe boundaries and validation ([444d7c9](https://github.com/graydonpleasants/geo-polygonize/commit/444d7c94999fc5551b543a7f08eb7325a99b3236))
* **core:** panic-safe boundaries and validation ([4c0b0c3](https://github.com/graydonpleasants/geo-polygonize/commit/4c0b0c336b52ea457289a332b9e42084535fb624))


### Bug Fixes

* **ci:** use time conditionally for wasm and fix python tests ([88d1242](https://github.com/graydonpleasants/geo-polygonize/commit/88d1242b6ecd1eb833d7f3d8adeea9e28ea3fad1))
* **core:** tighten determinism for dangles and ring rotation ([#270](https://github.com/graydonpleasants/geo-polygonize/issues/270)) ([1fcb3d8](https://github.com/graydonpleasants/geo-polygonize/commit/1fcb3d8476f0ecae1b9918c9141e3982441d2bb8))

## [0.8.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.7.0...geo-polygonize-v0.8.0) (2026-03-10)


### Features

* **core:** add diagnostics and report mode scaffold ([0b1ca3a](https://github.com/graydonpleasants/geo-polygonize/commit/0b1ca3a390da6e1a1b8870dc3d75fadd2f1e459b))
* **core:** implement diagnostics collection and fix tests ([#266](https://github.com/graydonpleasants/geo-polygonize/issues/266)) ([44ef7f0](https://github.com/graydonpleasants/geo-polygonize/commit/44ef7f04fae06e16cdcb5ea8d92d5ae30bbfcbad))
* **core:** implement typed Wasm and Python errors ([#273](https://github.com/graydonpleasants/geo-polygonize/issues/273)) ([f0e0b93](https://github.com/graydonpleasants/geo-polygonize/commit/f0e0b9351928a290dfe9c5a7154c834594d35402))
* **core:** Optimize SimdRing capacity allocation ([6e320d5](https://github.com/graydonpleasants/geo-polygonize/commit/6e320d502d39099a829e695de93843d176756849))
* **core:** Optimize SimdRing capacity allocation ([8696685](https://github.com/graydonpleasants/geo-polygonize/commit/8696685ed1d02f83b35b568d19dee29ae3744db5))
* **core:** Optimize SimdRing capacity allocation ([71ae67a](https://github.com/graydonpleasants/geo-polygonize/commit/71ae67ac33a17212c807b3c12771250a1a039d82))
* **core:** panic-safe boundaries and validation ([444d7c9](https://github.com/graydonpleasants/geo-polygonize/commit/444d7c94999fc5551b543a7f08eb7325a99b3236))
* **core:** panic-safe boundaries and validation ([4c0b0c3](https://github.com/graydonpleasants/geo-polygonize/commit/4c0b0c336b52ea457289a332b9e42084535fb624))


### Bug Fixes

* **ci:** use time conditionally for wasm and fix python tests ([88d1242](https://github.com/graydonpleasants/geo-polygonize/commit/88d1242b6ecd1eb833d7f3d8adeea9e28ea3fad1))
* **core:** apply ring rotation independently and fully order dangles ([202aa3a](https://github.com/graydonpleasants/geo-polygonize/commit/202aa3ac93a53989852e2772820bf38027384a17))
* **core:** enforce strict golden fixture assertions ([f6fba68](https://github.com/graydonpleasants/geo-polygonize/commit/f6fba6827ed3db8c95406e5102b4f0904c272576))
* **core:** enforce strict golden fixture assertions ([#272](https://github.com/graydonpleasants/geo-polygonize/issues/272)) ([fee9cd1](https://github.com/graydonpleasants/geo-polygonize/commit/fee9cd1c6f749fffbcaa1a8133e8f44fb26644bb))
* **core:** tighten determinism for dangles and ring rotation ([#270](https://github.com/graydonpleasants/geo-polygonize/issues/270)) ([1fcb3d8](https://github.com/graydonpleasants/geo-polygonize/commit/1fcb3d8476f0ecae1b9918c9141e3982441d2bb8))

## [0.7.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.6.3...geo-polygonize-v0.7.0) (2026-03-10)


### Features

* **core:** implement deterministic output and canonical sorting ([58e16d5](https://github.com/graydonpleasants/geo-polygonize/commit/58e16d552001963b9ed539dadf4cdbce82d90964))
* **core:** implement deterministic output and canonical sorting ([356de05](https://github.com/graydonpleasants/geo-polygonize/commit/356de0580601edebbab9e81a541267d47eba754b))
* **core:** implement diagnostics collection and fix tests ([#266](https://github.com/graydonpleasants/geo-polygonize/issues/266)) ([44ef7f0](https://github.com/graydonpleasants/geo-polygonize/commit/44ef7f04fae06e16cdcb5ea8d92d5ae30bbfcbad))
* **core:** implement typed Wasm and Python errors ([#273](https://github.com/graydonpleasants/geo-polygonize/issues/273)) ([f0e0b93](https://github.com/graydonpleasants/geo-polygonize/commit/f0e0b9351928a290dfe9c5a7154c834594d35402))


### Bug Fixes

* **core:** apply ring rotation independently and fully order dangles ([202aa3a](https://github.com/graydonpleasants/geo-polygonize/commit/202aa3ac93a53989852e2772820bf38027384a17))
* **core:** enforce strict golden fixture assertions ([f6fba68](https://github.com/graydonpleasants/geo-polygonize/commit/f6fba6827ed3db8c95406e5102b4f0904c272576))
* **core:** enforce strict golden fixture assertions ([#272](https://github.com/graydonpleasants/geo-polygonize/issues/272)) ([fee9cd1](https://github.com/graydonpleasants/geo-polygonize/commit/fee9cd1c6f749fffbcaa1a8133e8f44fb26644bb))
* **core:** tighten determinism for dangles and ring rotation ([#270](https://github.com/graydonpleasants/geo-polygonize/issues/270)) ([1fcb3d8](https://github.com/graydonpleasants/geo-polygonize/commit/1fcb3d8476f0ecae1b9918c9141e3982441d2bb8))

## [0.6.3](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.6.2...geo-polygonize-v0.6.3) (2026-03-08)


### Performance Improvements

* **core:** optimize area and centroid loop bounds-checking ([6e23dcc](https://github.com/graydonpleasants/geo-polygonize/commit/6e23dccda8c09cd455dd827d46c44f559991065d))

## [0.6.2](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.6.1...geo-polygonize-v0.6.2) (2026-03-07)


### Bug Fixes

* **wasm:** add repository url for npm provenance ([46736de](https://github.com/graydonpleasants/geo-polygonize/commit/46736deb7a73fe9f708aecfa8f4de944b9de6d33))

## [0.6.1](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.6.0...geo-polygonize-v0.6.1) (2026-03-07)


### Bug Fixes

* **core:** resolve clippy collapsible_if and thread_local lints ([635af15](https://github.com/graydonpleasants/geo-polygonize/commit/635af15f499472824999e2ff36d9ee1a4ae98fb1))
* **github:** upgrade npm before trusted npm publish ([bfab447](https://github.com/graydonpleasants/geo-polygonize/commit/bfab447544b64e06e99d09dcc432d7600b67ba54))


### Performance Improvements

* **core:** eliminate unnecessary clone of hole coordinates in assignment ([08a51ba](https://github.com/graydonpleasants/geo-polygonize/commit/08a51ba41543981fd870f12ccc4cb5109c0bda5a))
* **core:** optimize extract_segments by using iterative SmallVec stack and pre-allocating segment vectors ([6a8447b](https://github.com/graydonpleasants/geo-polygonize/commit/6a8447b12d4907d751f6b0162b039ec96e6e144d))
* **core:** optimize rings_share_edge evaluation and simd_shells filtering ([db8d3c1](https://github.com/graydonpleasants/geo-polygonize/commit/db8d3c1e75405fbe8e672a70f57e503e66789379))


### Build System

* **core:** fix formatting in extract_bench.rs ([85bf807](https://github.com/graydonpleasants/geo-polygonize/commit/85bf80760d7d0b7679d3a270bdc0087cf7800917))

## [0.6.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.5.0...geo-polygonize-v0.6.0) (2026-03-07)


### Features

* **python:** return SimplePolygon directly from PyO3 bindings ([#236](https://github.com/graydonpleasants/geo-polygonize/issues/236)) ([9977e68](https://github.com/graydonpleasants/geo-polygonize/commit/9977e6841c0157f1c5576235629b94b2cbe55c13))


### Performance Improvements

* **core:** Avoid cloning geometries inside TiledPolygonizer ([fdf513d](https://github.com/graydonpleasants/geo-polygonize/commit/fdf513d180e8c8a246963a78fc6cad09e6ca621e))
* **core:** Avoid cloning geometries inside TiledPolygonizer ([5f33624](https://github.com/graydonpleasants/geo-polygonize/commit/5f336249fb6048ed1ee12e9755e1322af05a7a7f))
* **core:** eager initialization of SimdRing objects over OnceLock ([628bfdd](https://github.com/graydonpleasants/geo-polygonize/commit/628bfdd7400c3bcf9be57c6f49eacc28b8ed0cad))

## [0.5.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.4.2...geo-polygonize-v0.5.0) (2026-03-07)


### Features

* **core:** optimize noding float sorting & dedup logic ([#232](https://github.com/graydonpleasants/geo-polygonize/issues/232)) ([a458c15](https://github.com/graydonpleasants/geo-polygonize/commit/a458c154e3d28f2a684b0f296cd72f218b56c2b1))
* **github:** add automerge workflow for graydonpleasants ([1f79ec0](https://github.com/graydonpleasants/geo-polygonize/commit/1f79ec0e67b45f25f7288ec057864d501a6ad89b))
* **github:** Add manual `workflow_dispatch` release with version input to release-please workflow ([#231](https://github.com/graydonpleasants/geo-polygonize/issues/231)) ([a8177ce](https://github.com/graydonpleasants/geo-polygonize/commit/a8177ce4b5a6e0b4b343221e61839287099fe25a))

## [0.4.2](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.4.1...geo-polygonize-v0.4.2) (2026-03-06)


### Performance Improvements

* **core:** avoid cloning geometries in tiling logic ([57eef63](https://github.com/graydonpleasants/geo-polygonize/commit/57eef63a550c19e403ac8ec65bbc29c19350248d))

## [0.4.1](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.4.0...geo-polygonize-v0.4.1) (2026-03-06)


### Bug Fixes

* **core:** remove debug print in polygonizer.rs ([6c77240](https://github.com/graydonpleasants/geo-polygonize/commit/6c772401865aa1d230e1c7fb1d1c14dfb07bc508))
* **github:** support release-please component tags in publish workflows ([#207](https://github.com/graydonpleasants/geo-polygonize/issues/207)) ([b59cccd](https://github.com/graydonpleasants/geo-polygonize/commit/b59cccd116c8225257d2bacdc1f78c6dee8fe93a))


### Performance Improvements

* **core:** Eliminate intermediate polygon allocations ([#204](https://github.com/graydonpleasants/geo-polygonize/issues/204)) ([5329162](https://github.com/graydonpleasants/geo-polygonize/commit/5329162172980c3f6bc154c3ef7a9986b7fe2838))

## [0.4.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.3.1...geo-polygonize-v0.4.0) (2026-03-06)


### Features

* **core:** trigger release for bounding_rect optimization ([854e4ce](https://github.com/graydonpleasants/geo-polygonize/commit/854e4ce1162d8f414e531624629d700df4b42318))
* **deps:** configure commitlint to strictly enforce conventional commits ([#203](https://github.com/graydonpleasants/geo-polygonize/issues/203)) ([23c6d13](https://github.com/graydonpleasants/geo-polygonize/commit/23c6d13020e4f45861657547e729dd06f93fd672))


### Bug Fixes

* **ci:** bump internal workspace dependencies during release-please sync ([#205](https://github.com/graydonpleasants/geo-polygonize/issues/205)) ([a26648c](https://github.com/graydonpleasants/geo-polygonize/commit/a26648c23d7f03137fa7c65dac3c265f58fb401d))

## [0.3.1](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.3.0...geo-polygonize-v0.3.1) (2026-03-06)


### Build System

* **github:** unhide build commits in release-please config ([#198](https://github.com/graydonpleasants/geo-polygonize/issues/198)) ([913fb9c](https://github.com/graydonpleasants/geo-polygonize/commit/913fb9c3ed932dac1c80f0da488d76201d3f753b))
* update geo-polygonize-core version in wasm Cargo.toml ([#196](https://github.com/graydonpleasants/geo-polygonize/issues/196)) ([0322e06](https://github.com/graydonpleasants/geo-polygonize/commit/0322e066c74a7f707196b3e2155c539d13b62f2d))
* **wasm:** update geo-polygonize-core version in Cargo.toml ([#197](https://github.com/graydonpleasants/geo-polygonize/issues/197)) ([d2a47c7](https://github.com/graydonpleasants/geo-polygonize/commit/d2a47c78034bed0d15b7649f42189118ad034f05))

## [0.3.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.2.1...geo-polygonize-v0.3.0) (2026-03-06)


### Features

* adaptive parallelism, fast math, and Wasm memory optimizations ([c46c370](https://github.com/graydonpleasants/geo-polygonize/commit/c46c370d23aca7a49060c520d8f1ecf79d48f5a8))
* Add PyO3 bindings and regenerate Cargo.lock ([a1e5602](https://github.com/graydonpleasants/geo-polygonize/commit/a1e560230e5bdc24d4c57dbdb696c76401c85c72))
* Add WASM SIMD support and publication enhancements ([015597e](https://github.com/graydonpleasants/geo-polygonize/commit/015597e58306a9d81523f89e44998fc4c24c0865))
* Add WASM SIMD support, feature detection, and publication pipeline ([32826e3](https://github.com/graydonpleasants/geo-polygonize/commit/32826e345b463473bff767f1a68d0a7bc7300b06))
* add webapp demo and wasm support ([f0548e7](https://github.com/graydonpleasants/geo-polygonize/commit/f0548e7e419962e882759c0bee2e428b43fc4ad7))
* Add zero-allocation area calculation methods to Polygon3D ([#155](https://github.com/graydonpleasants/geo-polygonize/issues/155)) ([c783c4a](https://github.com/graydonpleasants/geo-polygonize/commit/c783c4a85891dfb8d122c1925b837e7ca0fb3855))
* architectural optimizations (ISR, SIMD, geo-index) ([95e58ca](https://github.com/graydonpleasants/geo-polygonize/commit/95e58ca3de5ce93e7af620ea97809534daea0fb9))
* architectural optimizations (ISR, SIMD) ([3071bd7](https://github.com/graydonpleasants/geo-polygonize/commit/3071bd7f72b52ec4e20f49ccbfe5c00c9f5ee198))
* architectural optimizations (ISR, SIMD) and robust benchmarking ([f4df36d](https://github.com/graydonpleasants/geo-polygonize/commit/f4df36dc601371578be053aeefe5d474cb7446c3))
* architectural optimizations (ISR, SIMD) with robust fallbacks ([2760a09](https://github.com/graydonpleasants/geo-polygonize/commit/2760a094020a3e913a947d6bec3aa01b18503f3c))
* architectural optimizations (ISR, SIMD) with robust fallbacks and clean dependencies ([23cd067](https://github.com/graydonpleasants/geo-polygonize/commit/23cd067e61c23021e8d71d2dc25f07a29b125369))
* architectural optimizations (wasm benchmark, geoarrow, talc) ([b818bce](https://github.com/graydonpleasants/geo-polygonize/commit/b818bce6896d901696a542bc3c01c5a948a80c65))
* Capture invalid rings in Polygonizer result ([987a1a6](https://github.com/graydonpleasants/geo-polygonize/commit/987a1a66be62d43b43c1c67c1c6fb312b12f995e))
* Capture invalid rings in Polygonizer result ([4a9f685](https://github.com/graydonpleasants/geo-polygonize/commit/4a9f6854179148389c9a633d3fe9de11f8f6025d))
* **core:** Optimize geometry extraction by pre-allocating capacity using extend ([#172](https://github.com/graydonpleasants/geo-polygonize/issues/172)) ([922db67](https://github.com/graydonpleasants/geo-polygonize/commit/922db67c3540f6d15935579beaa82d1a7d503740))
* **core:** post benchmark results on PRs ([#176](https://github.com/graydonpleasants/geo-polygonize/issues/176)) ([b69e6a0](https://github.com/graydonpleasants/geo-polygonize/commit/b69e6a0418c2505d8f263e34f2849ba03222d107))
* Document and verify `valid_edges` hoisting optimization in PlanarGraph ([a0797fe](https://github.com/graydonpleasants/geo-polygonize/commit/a0797fed4a0174e0c5c84acf4612149dddc9b3a9))
* expand differential testing suite with 6 new generators ([ee7f983](https://github.com/graydonpleasants/geo-polygonize/commit/ee7f983dcc4eeb53e8145e02b0265ed1eb83e15f))
* implement spatial tiling and scale optimizations ([20ffdb4](https://github.com/graydonpleasants/geo-polygonize/commit/20ffdb4938bb0d8d86f54398864dc6ed47e64a7e))
* implement Uniform Grid noding and spatial sorting ([416a23a](https://github.com/graydonpleasants/geo-polygonize/commit/416a23a882830337a93739b4d2946375b4a0c5a7))
* Improve python bindings ergonomics and error handling ([#158](https://github.com/graydonpleasants/geo-polygonize/issues/158)) ([8d02115](https://github.com/graydonpleasants/geo-polygonize/commit/8d0211595eebfffea2a70ebc7b8871dbfa5b0559))
* Integrate FFI improvements and Python bindings ([25b210f](https://github.com/graydonpleasants/geo-polygonize/commit/25b210fa4c108d138fb26dd86d91f53477b257e3))
* memory pooling optimizations and parallel cfg fix ([17d0a8a](https://github.com/graydonpleasants/geo-polygonize/commit/17d0a8aafb22723f00e729b6728e9443e02a021a))
* optimize bulk data preparation with parallel iterators ([7650038](https://github.com/graydonpleasants/geo-polygonize/commit/76500389787e0dbb8a82816a72e2036e641cf922))
* **python:** add safety checks for 3D and odd-length coords in python wrapper ([f6b0ec4](https://github.com/graydonpleasants/geo-polygonize/commit/f6b0ec4f8780e15eb3a50cd089eaf56727da5476))
* Refactor Wasm build pipeline and exports ([667fb1f](https://github.com/graydonpleasants/geo-polygonize/commit/667fb1ffbd180c6801e4f9e5184bdba78ff76df5))
* Setup fleet issue automation with stacked PR support ([b4855fe](https://github.com/graydonpleasants/geo-polygonize/commit/b4855fe94a3e78b15b5d7249a4370bef48fe27ab))
* Setup fleet issue automation with stacked PR support ([abf1b66](https://github.com/graydonpleasants/geo-polygonize/commit/abf1b66406746a6701fcdef25f2d8b7941b15c39))
* Setup fleet issue automation with stacked PR support ([a988eff](https://github.com/graydonpleasants/geo-polygonize/commit/a988effd089490b4b38c610567adab79fb409f89))
* Setup fleet issue automation with stacked PR support ([#148](https://github.com/graydonpleasants/geo-polygonize/issues/148)) ([0996161](https://github.com/graydonpleasants/geo-polygonize/commit/0996161ba610424b787c33627c8f00b94c83892a))
* stress test noding and verify benchmarks ([afdf207](https://github.com/graydonpleasants/geo-polygonize/commit/afdf207bb034147cbb9a05af1b903af52ba0ba52))
* **test:** add robust WASM malformed JSON tests ([abe0614](https://github.com/graydonpleasants/geo-polygonize/commit/abe061416e1896a6c8dbee2cc407cf5acace2d95))
* Wasm optimizations, benchmarks, and docs ([71d8e85](https://github.com/graydonpleasants/geo-polygonize/commit/71d8e8588f525f877bb5f749f7d8f9d1f2a294b7))
* **wasm:** Expose explicit parity flags and document API ([#195](https://github.com/graydonpleasants/geo-polygonize/issues/195)) ([1271918](https://github.com/graydonpleasants/geo-polygonize/commit/12719182193a15b3cd97d55ed5cefdda736c45d7))


### Bug Fixes

* add missing version to geo-polygonize-core dependency ([9f8d31d](https://github.com/graydonpleasants/geo-polygonize/commit/9f8d31d2639deef94bb8c95c0629526f435d4253))
* Apply cargo fmt to resolve CI failure in ffi.rs ([f951b18](https://github.com/graydonpleasants/geo-polygonize/commit/f951b18212228814b455f1f56938261efa430982))
* Apply cargo fmt to src/polygonizer.rs ([791d994](https://github.com/graydonpleasants/geo-polygonize/commit/791d994573b9a9fc45442ad37f3b598cc079f61f))
* cargo fmt ([9996fad](https://github.com/graydonpleasants/geo-polygonize/commit/9996fad2fad65e2794d7843c969ef9333ba6f4ca))
* cargo fmt in benches ([668edec](https://github.com/graydonpleasants/geo-polygonize/commit/668edecd3ecca2e0a4217c9e95481011051277be))
* **core:** prevent potential panic by using safe zip iteration ([#186](https://github.com/graydonpleasants/geo-polygonize/issues/186)) ([29ce3ad](https://github.com/graydonpleasants/geo-polygonize/commit/29ce3ad9c6051f905e32629d60ea704d8d566783))
* **github:** track version updates for python and rust crates ([#192](https://github.com/graydonpleasants/geo-polygonize/issues/192)) ([35e5684](https://github.com/graydonpleasants/geo-polygonize/commit/35e56849174f1efd8e8a6f52bf2fb87e0049af53))
* **github:** track version updates for python and rust crates with manifest config ([#193](https://github.com/graydonpleasants/geo-polygonize/issues/193)) ([bf5a7a0](https://github.com/graydonpleasants/geo-polygonize/commit/bf5a7a0f1518224446960decb46831d759ecd52b))
* **github:** use trusted publishing for npm ([#181](https://github.com/graydonpleasants/geo-polygonize/issues/181)) ([57a47f3](https://github.com/graydonpleasants/geo-polygonize/commit/57a47f3a84ae9846d2dcc4f906643f26759f1513))
* Hyper-optimize Core Noding & Parallelism Framework ([4f2c557](https://github.com/graydonpleasants/geo-polygonize/commit/4f2c5573207f22a0330a9849a6679c857e29ee4f))
* Hyper-optimize Core Noding & Parallelism Framework ([53a7f6c](https://github.com/graydonpleasants/geo-polygonize/commit/53a7f6c15fe795a45c80cc92a3b849dccc9b3857))
* Improve Python error handling by checking CPolygonStatus ([7531506](https://github.com/graydonpleasants/geo-polygonize/commit/7531506f2ed5dc1053416515b88b3ff6e3818c1d))
* **python:** explicitly codesign macOS wheels to prevent EXC_BAD_ACCESS ([#190](https://github.com/graydonpleasants/geo-polygonize/issues/190)) ([c6129aa](https://github.com/graydonpleasants/geo-polygonize/commit/c6129aa651428fd732e7dcb477272075231f3aad))
* **python:** remove invalid `--release` argument from maturin publish command ([#179](https://github.com/graydonpleasants/geo-polygonize/issues/179)) ([b147494](https://github.com/graydonpleasants/geo-polygonize/commit/b147494649fd0a24e79061b9e747539ba8e0d11c))
* **python:** rename python package to geo-polygonize-py to avoid PyPI name collision ([#180](https://github.com/graydonpleasants/geo-polygonize/issues/180)) ([a24e862](https://github.com/graydonpleasants/geo-polygonize/commit/a24e86282117cdfc874e470c1ee7134b461d7718))
* Resolve active_cells unwrap panic ([d62f3bc](https://github.com/graydonpleasants/geo-polygonize/commit/d62f3bcb79a3d640facafa1a42ac9b7bd83c0970))
* resolve clippy warnings in noding/snap.rs ([03e41e2](https://github.com/graydonpleasants/geo-polygonize/commit/03e41e2ebea101de93c1d9ac6f0b6aa623416f0e))
* Resolve unwrap in `not(feature = "parallel")` path ([d70f234](https://github.com/graydonpleasants/geo-polygonize/commit/d70f234395fed98b0303e95ce36dd1285c9842a4))
* **security:** prevent panic in hole assignment ([9e27230](https://github.com/graydonpleasants/geo-polygonize/commit/9e272307df8d8ddf5bf0d120174eb7d1d0cc2dbe))
* use custom token for release-please to allow PR creation ([c11acb7](https://github.com/graydonpleasants/geo-polygonize/commit/c11acb7ed628313df18a770910797499fa9caa46))


### Performance Improvements

* **core:** optimize polygon construction loop by avoiding clones ([#187](https://github.com/graydonpleasants/geo-polygonize/issues/187)) ([8ae1e7d](https://github.com/graydonpleasants/geo-polygonize/commit/8ae1e7d0d347b7938538c055e321bb3e2c97d7e6))
* optimize hot loops by reusing vectors and avoiding clones ([9f600d8](https://github.com/graydonpleasants/geo-polygonize/commit/9f600d82e98967452655b640191bfc9ce97e7b4f))
* Optimize Polygonizer R-Tree allocation ([6322d6d](https://github.com/graydonpleasants/geo-polygonize/commit/6322d6dca95734c18eceafcdf4a5aa7ccee90347))
* Optimize SnapNoder by removing splits instead of cloning ([494469a](https://github.com/graydonpleasants/geo-polygonize/commit/494469a4a3b6b389f1a27d5eb73352d681cc274d))

## [0.3.1](https://github.com/graydonpleasants/geo-polygonize/compare/v0.3.0...v0.3.1) (2026-03-06)


### Bug Fixes

* **github:** track version updates for python and rust crates ([#192](https://github.com/graydonpleasants/geo-polygonize/issues/192)) ([35e5684](https://github.com/graydonpleasants/geo-polygonize/commit/35e56849174f1efd8e8a6f52bf2fb87e0049af53))
* **python:** explicitly codesign macOS wheels to prevent EXC_BAD_ACCESS ([#190](https://github.com/graydonpleasants/geo-polygonize/issues/190)) ([c6129aa](https://github.com/graydonpleasants/geo-polygonize/commit/c6129aa651428fd732e7dcb477272075231f3aad))

## [0.3.0](https://github.com/graydonpleasants/geo-polygonize/compare/v0.2.1...v0.3.0) (2026-03-06)


### Features

* **core:** post benchmark results on PRs ([#176](https://github.com/graydonpleasants/geo-polygonize/issues/176)) ([b69e6a0](https://github.com/graydonpleasants/geo-polygonize/commit/b69e6a0418c2505d8f263e34f2849ba03222d107))


### Bug Fixes

* **core:** prevent potential panic by using safe zip iteration ([#186](https://github.com/graydonpleasants/geo-polygonize/issues/186)) ([29ce3ad](https://github.com/graydonpleasants/geo-polygonize/commit/29ce3ad9c6051f905e32629d60ea704d8d566783))


### Performance Improvements

* **core:** optimize polygon construction loop by avoiding clones ([#187](https://github.com/graydonpleasants/geo-polygonize/issues/187)) ([8ae1e7d](https://github.com/graydonpleasants/geo-polygonize/commit/8ae1e7d0d347b7938538c055e321bb3e2c97d7e6))

## [0.2.1](https://github.com/graydonpleasants/geo-polygonize/compare/v0.2.0...v0.2.1) (2026-03-06)


### Bug Fixes

* **github:** use trusted publishing for npm ([#181](https://github.com/graydonpleasants/geo-polygonize/issues/181)) ([57a47f3](https://github.com/graydonpleasants/geo-polygonize/commit/57a47f3a84ae9846d2dcc4f906643f26759f1513))
* **python:** rename python package to geo-polygonize-py to avoid PyPI name collision ([#180](https://github.com/graydonpleasants/geo-polygonize/issues/180)) ([a24e862](https://github.com/graydonpleasants/geo-polygonize/commit/a24e86282117cdfc874e470c1ee7134b461d7718))

## [0.2.0](https://github.com/graydonpleasants/geo-polygonize/compare/v0.1.1...v0.2.0) (2026-03-06)


### Features

* **core:** Optimize geometry extraction by pre-allocating capacity using extend ([#172](https://github.com/graydonpleasants/geo-polygonize/issues/172)) ([922db67](https://github.com/graydonpleasants/geo-polygonize/commit/922db67c3540f6d15935579beaa82d1a7d503740))


### Bug Fixes

* add missing version to geo-polygonize-core dependency ([9f8d31d](https://github.com/graydonpleasants/geo-polygonize/commit/9f8d31d2639deef94bb8c95c0629526f435d4253))
* **python:** remove invalid `--release` argument from maturin publish command ([#179](https://github.com/graydonpleasants/geo-polygonize/issues/179)) ([b147494](https://github.com/graydonpleasants/geo-polygonize/commit/b147494649fd0a24e79061b9e747539ba8e0d11c))

## [0.1.1](https://github.com/graydonpleasants/geo-polygonize/compare/v0.1.0...v0.1.1) (2026-03-04)


### Bug Fixes

* Hyper-optimize Core Noding & Parallelism Framework ([4f2c557](https://github.com/graydonpleasants/geo-polygonize/commit/4f2c5573207f22a0330a9849a6679c857e29ee4f))
* Hyper-optimize Core Noding & Parallelism Framework ([53a7f6c](https://github.com/graydonpleasants/geo-polygonize/commit/53a7f6c15fe795a45c80cc92a3b849dccc9b3857))
* Resolve active_cells unwrap panic ([d62f3bc](https://github.com/graydonpleasants/geo-polygonize/commit/d62f3bcb79a3d640facafa1a42ac9b7bd83c0970))
* Resolve unwrap in `not(feature = "parallel")` path ([d70f234](https://github.com/graydonpleasants/geo-polygonize/commit/d70f234395fed98b0303e95ce36dd1285c9842a4))
* use custom token for release-please to allow PR creation ([c11acb7](https://github.com/graydonpleasants/geo-polygonize/commit/c11acb7ed628313df18a770910797499fa9caa46))

## [0.1.0](https://github.com/graydonpleasants/geo-polygonize/compare/v0.0.1...v0.1.0) (2026-03-04)


### Features

* Add PyO3 bindings and regenerate Cargo.lock ([a1e5602](https://github.com/graydonpleasants/geo-polygonize/commit/a1e560230e5bdc24d4c57dbdb696c76401c85c72))
* Add WASM SIMD support and publication enhancements ([015597e](https://github.com/graydonpleasants/geo-polygonize/commit/015597e58306a9d81523f89e44998fc4c24c0865))
* Add WASM SIMD support, feature detection, and publication pipeline ([32826e3](https://github.com/graydonpleasants/geo-polygonize/commit/32826e345b463473bff767f1a68d0a7bc7300b06))
* Add zero-allocation area calculation methods to Polygon3D ([#155](https://github.com/graydonpleasants/geo-polygonize/issues/155)) ([c783c4a](https://github.com/graydonpleasants/geo-polygonize/commit/c783c4a85891dfb8d122c1925b837e7ca0fb3855))
* architectural optimizations (ISR, SIMD, geo-index) ([95e58ca](https://github.com/graydonpleasants/geo-polygonize/commit/95e58ca3de5ce93e7af620ea97809534daea0fb9))
* architectural optimizations (ISR, SIMD) ([3071bd7](https://github.com/graydonpleasants/geo-polygonize/commit/3071bd7f72b52ec4e20f49ccbfe5c00c9f5ee198))
* architectural optimizations (ISR, SIMD) and robust benchmarking ([f4df36d](https://github.com/graydonpleasants/geo-polygonize/commit/f4df36dc601371578be053aeefe5d474cb7446c3))
* architectural optimizations (ISR, SIMD) with robust fallbacks ([2760a09](https://github.com/graydonpleasants/geo-polygonize/commit/2760a094020a3e913a947d6bec3aa01b18503f3c))
* architectural optimizations (ISR, SIMD) with robust fallbacks and clean dependencies ([23cd067](https://github.com/graydonpleasants/geo-polygonize/commit/23cd067e61c23021e8d71d2dc25f07a29b125369))
* Capture invalid rings in Polygonizer result ([987a1a6](https://github.com/graydonpleasants/geo-polygonize/commit/987a1a66be62d43b43c1c67c1c6fb312b12f995e))
* Capture invalid rings in Polygonizer result ([4a9f685](https://github.com/graydonpleasants/geo-polygonize/commit/4a9f6854179148389c9a633d3fe9de11f8f6025d))
* Document and verify `valid_edges` hoisting optimization in PlanarGraph ([a0797fe](https://github.com/graydonpleasants/geo-polygonize/commit/a0797fed4a0174e0c5c84acf4612149dddc9b3a9))
* expand differential testing suite with 6 new generators ([ee7f983](https://github.com/graydonpleasants/geo-polygonize/commit/ee7f983dcc4eeb53e8145e02b0265ed1eb83e15f))
* implement Uniform Grid noding and spatial sorting ([416a23a](https://github.com/graydonpleasants/geo-polygonize/commit/416a23a882830337a93739b4d2946375b4a0c5a7))
* Improve python bindings ergonomics and error handling ([#158](https://github.com/graydonpleasants/geo-polygonize/issues/158)) ([8d02115](https://github.com/graydonpleasants/geo-polygonize/commit/8d0211595eebfffea2a70ebc7b8871dbfa5b0559))
* Integrate FFI improvements and Python bindings ([25b210f](https://github.com/graydonpleasants/geo-polygonize/commit/25b210fa4c108d138fb26dd86d91f53477b257e3))
* **python:** add safety checks for 3D and odd-length coords in python wrapper ([f6b0ec4](https://github.com/graydonpleasants/geo-polygonize/commit/f6b0ec4f8780e15eb3a50cd089eaf56727da5476))
* Refactor Wasm build pipeline and exports ([667fb1f](https://github.com/graydonpleasants/geo-polygonize/commit/667fb1ffbd180c6801e4f9e5184bdba78ff76df5))
* Setup fleet issue automation with stacked PR support ([b4855fe](https://github.com/graydonpleasants/geo-polygonize/commit/b4855fe94a3e78b15b5d7249a4370bef48fe27ab))
* Setup fleet issue automation with stacked PR support ([abf1b66](https://github.com/graydonpleasants/geo-polygonize/commit/abf1b66406746a6701fcdef25f2d8b7941b15c39))
* Setup fleet issue automation with stacked PR support ([a988eff](https://github.com/graydonpleasants/geo-polygonize/commit/a988effd089490b4b38c610567adab79fb409f89))
* Setup fleet issue automation with stacked PR support ([#148](https://github.com/graydonpleasants/geo-polygonize/issues/148)) ([0996161](https://github.com/graydonpleasants/geo-polygonize/commit/0996161ba610424b787c33627c8f00b94c83892a))
* stress test noding and verify benchmarks ([afdf207](https://github.com/graydonpleasants/geo-polygonize/commit/afdf207bb034147cbb9a05af1b903af52ba0ba52))
* **test:** add robust WASM malformed JSON tests ([abe0614](https://github.com/graydonpleasants/geo-polygonize/commit/abe061416e1896a6c8dbee2cc407cf5acace2d95))


### Bug Fixes

* Apply cargo fmt to resolve CI failure in ffi.rs ([f951b18](https://github.com/graydonpleasants/geo-polygonize/commit/f951b18212228814b455f1f56938261efa430982))
* Apply cargo fmt to src/polygonizer.rs ([791d994](https://github.com/graydonpleasants/geo-polygonize/commit/791d994573b9a9fc45442ad37f3b598cc079f61f))
* cargo fmt ([9996fad](https://github.com/graydonpleasants/geo-polygonize/commit/9996fad2fad65e2794d7843c969ef9333ba6f4ca))
* cargo fmt in benches ([668edec](https://github.com/graydonpleasants/geo-polygonize/commit/668edecd3ecca2e0a4217c9e95481011051277be))
* Improve Python error handling by checking CPolygonStatus ([7531506](https://github.com/graydonpleasants/geo-polygonize/commit/7531506f2ed5dc1053416515b88b3ff6e3818c1d))
* resolve clippy warnings in noding/snap.rs ([03e41e2](https://github.com/graydonpleasants/geo-polygonize/commit/03e41e2ebea101de93c1d9ac6f0b6aa623416f0e))
* **security:** prevent panic in hole assignment ([9e27230](https://github.com/graydonpleasants/geo-polygonize/commit/9e272307df8d8ddf5bf0d120174eb7d1d0cc2dbe))


### Performance Improvements

* optimize hot loops by reusing vectors and avoiding clones ([9f600d8](https://github.com/graydonpleasants/geo-polygonize/commit/9f600d82e98967452655b640191bfc9ce97e7b4f))
* Optimize Polygonizer R-Tree allocation ([6322d6d](https://github.com/graydonpleasants/geo-polygonize/commit/6322d6dca95734c18eceafcdf4a5aa7ccee90347))
* Optimize SnapNoder by removing splits instead of cloning ([494469a](https://github.com/graydonpleasants/geo-polygonize/commit/494469a4a3b6b389f1a27d5eb73352d681cc274d))

## [0.1.0] - 2024-05-22

### Added
- Initial release.
- Robust polygonization algorithm.
- Iterated Snap Rounding (ISR) noding.
- SIMD acceleration for hole assignment.
- WebAssembly support with `talc` allocator.
- Tiled polygonization for scalability.
