# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
