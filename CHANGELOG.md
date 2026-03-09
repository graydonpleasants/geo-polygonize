# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.8.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.7.0...geo-polygonize-v0.8.0) (2026-03-09)


### Features

* **core:** implement diagnostics collection and fix tests ([#266](https://github.com/graydonpleasants/geo-polygonize/issues/266)) ([44ef7f0](https://github.com/graydonpleasants/geo-polygonize/commit/44ef7f04fae06e16cdcb5ea8d92d5ae30bbfcbad))


### Bug Fixes

* **core:** apply ring rotation independently and fully order dangles ([202aa3a](https://github.com/graydonpleasants/geo-polygonize/commit/202aa3ac93a53989852e2772820bf38027384a17))
* **core:** enforce strict golden fixture assertions ([f6fba68](https://github.com/graydonpleasants/geo-polygonize/commit/f6fba6827ed3db8c95406e5102b4f0904c272576))
* **core:** enforce strict golden fixture assertions ([#272](https://github.com/graydonpleasants/geo-polygonize/issues/272)) ([fee9cd1](https://github.com/graydonpleasants/geo-polygonize/commit/fee9cd1c6f749fffbcaa1a8133e8f44fb26644bb))

## [0.7.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.6.3...geo-polygonize-v0.7.0) (2026-03-09)


### Features

* **core:** implement deterministic output and canonical sorting ([58e16d5](https://github.com/graydonpleasants/geo-polygonize/commit/58e16d552001963b9ed539dadf4cdbce82d90964))
* **core:** implement deterministic output and canonical sorting ([356de05](https://github.com/graydonpleasants/geo-polygonize/commit/356de0580601edebbab9e81a541267d47eba754b))

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
