# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.1.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v1.0.0...geo-polygonize-v1.1.0) (2026-08-20)


### Features

* **core:** add detached extraction gate ([#1367](https://github.com/graydonpleasants/geo-polygonize/issues/1367)) ([879db48](https://github.com/graydonpleasants/geo-polygonize/commit/879db48ce0b3b8915385668d2b7bb0616883e8dd))
* **core:** add private extraction readiness gate ([#1377](https://github.com/graydonpleasants/geo-polygonize/issues/1377)) ([541e710](https://github.com/graydonpleasants/geo-polygonize/commit/541e710d4a57aacebffe3b48651ae9f815883c86))
* **core:** add private face invariant gate ([#1379](https://github.com/graydonpleasants/geo-polygonize/issues/1379)) ([73a14f7](https://github.com/graydonpleasants/geo-polygonize/commit/73a14f77952b0b2b4eac0e32c79303ed977806a9))
* **core:** apply qualified partition twin links ([7cc03c1](https://github.com/graydonpleasants/geo-polygonize/commit/7cc03c1e130cb39d32ddfaa189b69f0b5ab2c439))
* **core:** assemble detached ring candidates ([#1370](https://github.com/graydonpleasants/geo-polygonize/issues/1370)) ([6ec89a6](https://github.com/graydonpleasants/geo-polygonize/commit/6ec89a608d7338889f9b0defa83c8ddcfe74481e))
* **core:** classify detached ring evidence ([#1369](https://github.com/graydonpleasants/geo-polygonize/issues/1369)) ([46ef86c](https://github.com/graydonpleasants/geo-polygonize/commit/46ef86c00bb211cef35f0d8168b219b3b966e04f))
* **core:** combine global topology evidence gates ([292baa8](https://github.com/graydonpleasants/geo-polygonize/commit/292baa86410c96726cc23e22779622cba7ca6a3f))
* **core:** commit detached global face IDs ([a1a284e](https://github.com/graydonpleasants/geo-polygonize/commit/a1a284ecc1509db14b486fa468fb49946a9f5551))
* **core:** commit gated detached successor links ([3d48355](https://github.com/graydonpleasants/geo-polygonize/commit/3d483553c35cd66f1eecc7dc75735de8e2bd8ef5))
* **core:** commit private extraction snapshot ([#1380](https://github.com/graydonpleasants/geo-polygonize/issues/1380)) ([d23fec3](https://github.com/graydonpleasants/geo-polygonize/commit/d23fec39b9d630947c5be939b2a43f27f38a6d9c))
* **core:** compare stitched output with untiled ([#1383](https://github.com/graydonpleasants/geo-polygonize/issues/1383)) ([a9ccad3](https://github.com/graydonpleasants/geo-polygonize/commit/a9ccad39b0eb2a7c98dd1efc5c604d31db012e2a))
* **core:** differential-fuzz tiled equivalence ([#1384](https://github.com/graydonpleasants/geo-polygonize/issues/1384)) ([72584c7](https://github.com/graydonpleasants/geo-polygonize/commit/72584c7af24f3591f4f30ea1aa6485d0f484642e))
* **core:** export atomic border observations ([40c7d71](https://github.com/graydonpleasants/geo-polygonize/commit/40c7d7163948fb6a201c3d8a5aefd3c594cbe264))
* **core:** export tile border observations ([#1279](https://github.com/graydonpleasants/geo-polygonize/issues/1279)) ([bad8182](https://github.com/graydonpleasants/geo-polygonize/commit/bad8182a3e98e013072635edafb81a0510515388))
* **core:** expose validated stitched output ([#1382](https://github.com/graydonpleasants/geo-polygonize/issues/1382)) ([4f02929](https://github.com/graydonpleasants/geo-polygonize/commit/4f0292904d20999b5b78ad2e142eb0801f7e1516))
* **core:** gate detached cycle face promotion ([#1361](https://github.com/graydonpleasants/geo-polygonize/issues/1361)) ([39e458f](https://github.com/graydonpleasants/geo-polygonize/commit/39e458ff96b16799ac14c34033b07d0f8d5cded9))
* **core:** gate detached ring extraction readiness ([#1372](https://github.com/graydonpleasants/geo-polygonize/issues/1372)) ([b925c90](https://github.com/graydonpleasants/geo-polygonize/commit/b925c902d2ecf83c923ad22997fac056bad35e20))
* **core:** gate global face next application ([f1413b9](https://github.com/graydonpleasants/geo-polygonize/commit/f1413b96ae4689d1226cc9e6dec66c3a00ca8ec9))
* **core:** gate global face transitions ([e44fca7](https://github.com/graydonpleasants/geo-polygonize/commit/e44fca705aa9934ec412f2ad0f79279a14a9714e))
* **core:** gate global topology application ([f4bdf6a](https://github.com/graydonpleasants/geo-polygonize/commit/f4bdf6a29a41f7a601adb463d59f1f2cc61c1bf8))
* **core:** gate global unbounded face evidence ([d587e41](https://github.com/graydonpleasants/geo-polygonize/commit/d587e41d942fd228ef485c23d810f64829c18463))
* **core:** gate global unbounded face proof ([e8b38fa](https://github.com/graydonpleasants/geo-polygonize/commit/e8b38fac3dc599b65461c3e45db7005674a85c4c))
* **core:** integrate detached face identity invariants ([#1356](https://github.com/graydonpleasants/geo-polygonize/issues/1356)) ([6e65db7](https://github.com/graydonpleasants/geo-polygonize/commit/6e65db71721216e8f2e16f5e3b3ba64c5b8a0193))
* **core:** integrate detached global next lineage ([#1357](https://github.com/graydonpleasants/geo-polygonize/issues/1357)) ([5a4acd2](https://github.com/graydonpleasants/geo-polygonize/commit/5a4acd2e01816656395535973ffba53a9f873cdf))
* **core:** map cross-partition face transitions ([f02dff0](https://github.com/graydonpleasants/geo-polygonize/commit/f02dff06656ab070ebf9409fc129dbde5a2bae41))
* **core:** map global face edge lineage ([cbc32f7](https://github.com/graydonpleasants/geo-polygonize/commit/cbc32f7988934d7f0ff5d708e8fd6a24d2d71a6c))
* **core:** materialize detached global face identity ([a3e8757](https://github.com/graydonpleasants/geo-polygonize/commit/a3e875796f16b30a2e1ac5dd53174cb605557e4d))
* **core:** materialize detached ring payloads ([#1368](https://github.com/graydonpleasants/geo-polygonize/issues/1368)) ([bfcde0a](https://github.com/graydonpleasants/geo-polygonize/commit/bfcde0a3921f5f7898cb0a1be8a6debb57c49da2))
* **core:** materialize private global face topology ([#1375](https://github.com/graydonpleasants/geo-polygonize/issues/1375)) ([8db5bce](https://github.com/graydonpleasants/geo-polygonize/commit/8db5bce1342a67687c70ef7947d736c876f0bcd0))
* **core:** materialize private ring payloads ([#1373](https://github.com/graydonpleasants/geo-polygonize/issues/1373)) ([d68e950](https://github.com/graydonpleasants/geo-polygonize/commit/d68e950261aa26e35097d60375e846274877591d))
* **core:** materialize private unbounded face topology ([#1376](https://github.com/graydonpleasants/geo-polygonize/issues/1376)) ([b108869](https://github.com/graydonpleasants/geo-polygonize/commit/b108869aecfe3795238cdded2eb725338d54a488))
* **core:** physically node partition boundaries ([51abedc](https://github.com/graydonpleasants/geo-polygonize/commit/51abedc9f3959d8e4236fbc7bef5de31618c0801))
* **core:** plan deterministic global face ids ([ad46d2e](https://github.com/graydonpleasants/geo-polygonize/commit/ad46d2e065b7ea1cffab24c281fe17df5a0f19e7))
* **core:** plan global face boundaries ([3fd0f0f](https://github.com/graydonpleasants/geo-polygonize/commit/3fd0f0faa1a4d8a335c1fc0c7a5da397def9d0c8))
* **core:** plan global face identities ([33f5e94](https://github.com/graydonpleasants/geo-polygonize/commit/33f5e9426ae8b77e50eee93b89847ecf184969e9))
* **core:** plan global face next application ([9ca460a](https://github.com/graydonpleasants/geo-polygonize/commit/9ca460a0a4eb4f367cb39d2f71d172f2c4a6bdc9))
* **core:** plan global face next candidates ([ffab129](https://github.com/graydonpleasants/geo-polygonize/commit/ffab12994f278cec9af33cbd26126c858d6f1d59))
* **core:** plan global face payload merges ([bebfd41](https://github.com/graydonpleasants/geo-polygonize/commit/bebfd417d93d920993308a541caeb734f09c00f6))
* **core:** plan global face transitions ([fc488e9](https://github.com/graydonpleasants/geo-polygonize/commit/fc488e9cc2891bb8a03446aab0d49523ab21d5f0))
* **core:** promote detached unbounded face identity ([1ac41ae](https://github.com/graydonpleasants/geo-polygonize/commit/1ac41ae6c1e71ac44aebd1344cf66668e8377483))
* **core:** reconcile global border components ([65cf5a0](https://github.com/graydonpleasants/geo-polygonize/commit/65cf5a00648faa9e885e65a14f9296281ed404a3))
* **core:** reconcile global face nodes ([3f54bfd](https://github.com/graydonpleasants/geo-polygonize/commit/3f54bfdeeeaaeae1f6a4d310809ed383bd8f36b8))
* **core:** reconcile partition border nodes ([9d4969f](https://github.com/graydonpleasants/geo-polygonize/commit/9d4969f225eff0980b07ef57370115367a094a3a))
* **core:** report declared partition twins ([85dc102](https://github.com/graydonpleasants/geo-polygonize/commit/85dc1024d706a6642a2c795f61c053eaf3849d7b))
* **core:** retain executable source-chain identity ([#1276](https://github.com/graydonpleasants/geo-polygonize/issues/1276)) ([7eb2b63](https://github.com/graydonpleasants/geo-polygonize/commit/7eb2b6311607b1205755918f7036f632b931618f))
* **core:** retain non-polygon extraction evidence ([#1374](https://github.com/graydonpleasants/geo-polygonize/issues/1374)) ([1d26adb](https://github.com/graydonpleasants/geo-polygonize/commit/1d26adb40acf1145f51b70661b697b0e76c79383))
* **core:** retain partition border payload identities ([109cae1](https://github.com/graydonpleasants/geo-polygonize/commit/109cae16b21061623a0d25e6b1db15d3b3c5f537))
* **core:** trace partition boundary evidence ([aef6be5](https://github.com/graydonpleasants/geo-polygonize/commit/aef6be537751f7db0fd4ca904b2e5163abb3ed36))
* **core:** validate canonical border node payloads ([84e7943](https://github.com/graydonpleasants/geo-polygonize/commit/84e7943638633266b79dc4f5153cea2da3461fca))
* **core:** validate detached cycle face lineage ([#1360](https://github.com/graydonpleasants/geo-polygonize/issues/1360)) ([ac7b82f](https://github.com/graydonpleasants/geo-polygonize/commit/ac7b82f55416ce541c0620e97cf007ac4f068a96))
* **core:** validate detached cycle geometry ([#1363](https://github.com/graydonpleasants/geo-polygonize/issues/1363)) ([9911348](https://github.com/graydonpleasants/geo-polygonize/commit/99113487e6a1529c12a60d42b5b08327e9b2dfe1))
* **core:** validate detached cycle interactions ([9caa34b](https://github.com/graydonpleasants/geo-polygonize/commit/9caa34b24394463492d092dbc2abc5ef9d392fd1))
* **core:** validate detached face payload lineage ([#1362](https://github.com/graydonpleasants/geo-polygonize/issues/1362)) ([08346fa](https://github.com/graydonpleasants/geo-polygonize/commit/08346fa4c7f61daeee5d1a0da531a51452a419bc))
* **core:** validate detached ring payloads ([#1366](https://github.com/graydonpleasants/geo-polygonize/issues/1366)) ([c590d33](https://github.com/graydonpleasants/geo-polygonize/commit/c590d3319ee57bd5f4a13a6fa73b6803ec0107c4))
* **core:** validate global component coverage ([6ab7549](https://github.com/graydonpleasants/geo-polygonize/commit/6ab7549e32839d9d3665adfaf507aa91b3add074))
* **core:** validate global face boundaries ([f38d826](https://github.com/graydonpleasants/geo-polygonize/commit/f38d826dbc73294961f234d737d0244dd1174703))
* **core:** validate global face ID application ([eb8f975](https://github.com/graydonpleasants/geo-polygonize/commit/eb8f9750f76c89ffa61191dfcbe6438708d26c0c))
* **core:** validate global face walk invariants ([f39207a](https://github.com/graydonpleasants/geo-polygonize/commit/f39207a9f74f88fafbb4f56ceb8a4f7d06d378d3))
* **core:** validate global topology candidate ([5864f26](https://github.com/graydonpleasants/geo-polygonize/commit/5864f26780e442b6eb006659cecd0d3c35acd13a))
* **core:** witness global face Euler boundary ([a9240ef](https://github.com/graydonpleasants/geo-polygonize/commit/a9240efdf4e7c3d4e20e1d8b03e8965ec617c833))
* **github:** verify cross-registry release publication ([#1293](https://github.com/graydonpleasants/geo-polygonize/issues/1293)) ([437a7c0](https://github.com/graydonpleasants/geo-polygonize/commit/437a7c0f2fa37f1049d8c4d625b7660a3af48a9a))


### Bug Fixes

* **core:** allowlist component memory diagnostics ([cda28d2](https://github.com/graydonpleasants/geo-polygonize/commit/cda28d21da7913844b692dbab0167b772949dc83))
* **core:** keep boundary contract helper test-local ([975ef0a](https://github.com/graydonpleasants/geo-polygonize/commit/975ef0ae31db8b40de74e9ee80c3e9b607d863d5))
* **core:** preserve diagnostics source compatibility ([#1395](https://github.com/graydonpleasants/geo-polygonize/issues/1395)) ([bdb4b5a](https://github.com/graydonpleasants/geo-polygonize/commit/bdb4b5a03433796998604dd250081c6adc041e37))
* **core:** reject conflicting border observations ([#1271](https://github.com/graydonpleasants/geo-polygonize/issues/1271)) ([6b48cc8](https://github.com/graydonpleasants/geo-polygonize/commit/6b48cc85d3bb5fd51c09901109b7b4c030c1814b))
* **core:** require partition border adjacency ([#1272](https://github.com/graydonpleasants/geo-polygonize/issues/1272)) ([af70e11](https://github.com/graydonpleasants/geo-polygonize/commit/af70e1180c2effd60e3780d568c2e204c5768e22))
* **core:** satisfy boundary intersection lint ([2882dc6](https://github.com/graydonpleasants/geo-polygonize/commit/2882dc66f86bddecafe4e9a188d428235acf61d9))
* **wasm:** verify wasm-opt is executable before use ([#1365](https://github.com/graydonpleasants/geo-polygonize/issues/1365)) ([c1d624c](https://github.com/graydonpleasants/geo-polygonize/commit/c1d624c56768c6276ee5351b64c4d69d33cb736f))


### Performance Improvements

* **core:** cache MCIndex monotone-chain envelopes ([43e49e7](https://github.com/graydonpleasants/geo-polygonize/commit/43e49e75236497310c6a96461f8c970f561e05c1))
* **core:** cover hybrid MCIndex source chains ([612b01c](https://github.com/graydonpleasants/geo-polygonize/commit/612b01c889bba55f96001973d899131000668a69))
* **core:** evaluate hybrid candidates exactly ([1b8eb60](https://github.com/graydonpleasants/geo-polygonize/commit/1b8eb601a944aff5b07cd19389ad16c5a005711b))
* **core:** expose component memory evidence ([b13d4b4](https://github.com/graydonpleasants/geo-polygonize/commit/b13d4b42f22b10f0ad476ff2c183bffcc1d0a652))
* **core:** preserve MCIndex segment identity ([3924f4d](https://github.com/graydonpleasants/geo-polygonize/commit/3924f4d6207a0777d6eb514be4bda3499d5571b3))
* **core:** stream MCIndex candidates through visitor ([b8f7028](https://github.com/graydonpleasants/geo-polygonize/commit/b8f7028fe735b8d8f0087e7949a45e6dce9e1d6a))

## [1.0.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.76.2...geo-polygonize-v1.0.0) (2026-08-03)


### ⚠ BREAKING CHANGES

* **core:** remove unreachable error variants ([#1124](https://github.com/graydonpleasants/geo-polygonize/issues/1124))
* **core:** remove retired advanced noder alias ([#1121](https://github.com/graydonpleasants/geo-polygonize/issues/1121))

### Features

* **core:** add partition-border graph representation ([#1243](https://github.com/graydonpleasants/geo-polygonize/issues/1243)) ([db2c3dc](https://github.com/graydonpleasants/geo-polygonize/commit/db2c3dcbe15a874f0c29d4454d7b4e722aa7aac0))
* **core:** add per-stage trace byte limits ([#1039](https://github.com/graydonpleasants/geo-polygonize/issues/1039)) ([299b92a](https://github.com/graydonpleasants/geo-polygonize/commit/299b92a08e7eb30986d502a8add23aaa63307ba2))
* **core:** add safe component fallback ([#1170](https://github.com/graydonpleasants/geo-polygonize/issues/1170)) ([4609b38](https://github.com/graydonpleasants/geo-polygonize/commit/4609b380b97370e6b34421cbcb23b53879050bba))
* **core:** add safe untiled fallback for tiled coverage ([#1156](https://github.com/graydonpleasants/geo-polygonize/issues/1156)) ([3250835](https://github.com/graydonpleasants/geo-polygonize/commit/325083595a5d67a32a050a8e4adb1913ea31ba12))
* **core:** add workload descriptors ([#1251](https://github.com/graydonpleasants/geo-polygonize/issues/1251)) ([701d137](https://github.com/graydonpleasants/geo-polygonize/commit/701d137295f87488bad3807da8df6dad0df9d15a))
* **core:** assign deterministic face identities ([#1240](https://github.com/graydonpleasants/geo-polygonize/issues/1240)) ([7c2988e](https://github.com/graydonpleasants/geo-polygonize/commit/7c2988efbc8eb01a07d4acd84aaa0852f3847e38))
* **core:** bound tiled retry attempts ([#1232](https://github.com/graydonpleasants/geo-polygonize/issues/1232)) ([a1afeed](https://github.com/graydonpleasants/geo-polygonize/commit/a1afeed30a060cac7917e00e747de8f4a7b74d0c))
* **core:** classify declined fallback traces ([#1195](https://github.com/graydonpleasants/geo-polygonize/issues/1195)) ([1587f6b](https://github.com/graydonpleasants/geo-polygonize/commit/1587f6b59dcafa87e3b5b581a4e8a459a2b64d6a))
* **core:** complete partition-border graph representation ([#1253](https://github.com/graydonpleasants/geo-polygonize/issues/1253)) ([00a6528](https://github.com/graydonpleasants/geo-polygonize/commit/00a652830afaff09a616ae2380f0b8ad2d7cccb0))
* **core:** define bounded trace schema ([#1018](https://github.com/graydonpleasants/geo-polygonize/issues/1018)) ([aeed858](https://github.com/graydonpleasants/geo-polygonize/commit/aeed8589cde447178b9d8275a62cd6beadba5541))
* **core:** enforce tiled owned-face coverage ([#1132](https://github.com/graydonpleasants/geo-polygonize/issues/1132)) ([d3006c0](https://github.com/graydonpleasants/geo-polygonize/commit/d3006c0c8578eedddfd7846049488fce54cd477c))
* **core:** export minimized compatibility fixtures ([#1044](https://github.com/graydonpleasants/geo-polygonize/issues/1044)) ([ab648b0](https://github.com/graydonpleasants/geo-polygonize/commit/ab648b0fa854638191c9a76061831946d19e4e44))
* **core:** expose component fallback counts ([#1188](https://github.com/graydonpleasants/geo-polygonize/issues/1188)) ([3910d99](https://github.com/graydonpleasants/geo-polygonize/commit/3910d9944b3397ea82220dcfa87a8d913019121b))
* **core:** expose tiled fallback merge counts ([#1182](https://github.com/graydonpleasants/geo-polygonize/issues/1182)) ([221cf33](https://github.com/graydonpleasants/geo-polygonize/commit/221cf33aa2fadd966c5f1a7676d931a3d22c32c1))
* **core:** merge disjoint tiled fallback regions ([#1177](https://github.com/graydonpleasants/geo-polygonize/issues/1177)) ([02ce6be](https://github.com/graydonpleasants/geo-polygonize/commit/02ce6bec42c33eba6e501bec52e6ad7e38e34471))
* **core:** persist directed-edge next links ([#1239](https://github.com/graydonpleasants/geo-polygonize/issues/1239)) ([ed554bd](https://github.com/graydonpleasants/geo-polygonize/commit/ed554bd99a84224a5c882ed694731a107b4373e1))
* **core:** preserve tiled fallback error reasons ([#1206](https://github.com/graydonpleasants/geo-polygonize/issues/1206)) ([d2ac576](https://github.com/graydonpleasants/geo-polygonize/commit/d2ac57647fe64ad1c8baeda44246342643fde81d))
* **core:** process graph components locally ([#1241](https://github.com/graydonpleasants/geo-polygonize/issues/1241)) ([69065ba](https://github.com/graydonpleasants/geo-polygonize/commit/69065ba1addce002d222bc0fa90dd454cad45af6))
* **core:** prototype monotone-chain candidates ([#1260](https://github.com/graydonpleasants/geo-polygonize/issues/1260)) ([0bfd28d](https://github.com/graydonpleasants/geo-polygonize/commit/0bfd28d92f01f088870401bb4afc7cda924d5f66))
* **core:** prototype sweep intersection backend ([#1259](https://github.com/graydonpleasants/geo-polygonize/issues/1259)) ([920c97c](https://github.com/graydonpleasants/geo-polygonize/commit/920c97c68fb15fb5f3e51537a2ffacae5900a560))
* **core:** record debugger diagnostics in traces ([39357a3](https://github.com/graydonpleasants/geo-polygonize/commit/39357a370db4a6cc5cb174c67e24e91e28a37b2a))
* **core:** recover closed tiled boundary regions ([#1208](https://github.com/graydonpleasants/geo-polygonize/issues/1208)) ([ee8b343](https://github.com/graydonpleasants/geo-polygonize/commit/ee8b3431c7dcdc0a7ae6c06894d86b488cca8c15))
* **core:** recover interacting tiled regions ([#1189](https://github.com/graydonpleasants/geo-polygonize/issues/1189)) ([fab7aea](https://github.com/graydonpleasants/geo-polygonize/commit/fab7aea8654d0b45da7cc3c723be26ad96849ecf))
* **core:** recover mixed tiled component evidence ([#1211](https://github.com/graydonpleasants/geo-polygonize/issues/1211)) ([4a70c4e](https://github.com/graydonpleasants/geo-polygonize/commit/4a70c4e60cb580ce3e9548edd497f8be56e90b37))
* **core:** recover partially observed components ([#1192](https://github.com/graydonpleasants/geo-polygonize/issues/1192)) ([9ee311d](https://github.com/graydonpleasants/geo-polygonize/commit/9ee311dc19c7a03df69d87a536d247255c4922ba))
* **core:** remove retired advanced noder alias ([#1121](https://github.com/graydonpleasants/geo-polygonize/issues/1121)) ([8935f94](https://github.com/graydonpleasants/geo-polygonize/commit/8935f94ea10470607ef285084781dbb1b56a9330))
* **core:** remove unreachable error variants ([#1124](https://github.com/graydonpleasants/geo-polygonize/issues/1124)) ([e7e63e7](https://github.com/graydonpleasants/geo-polygonize/commit/e7e63e724f2cadc08b3a485703743161d0c5f465))
* **core:** report certified fixed-grid components ([#1184](https://github.com/graydonpleasants/geo-polygonize/issues/1184)) ([d359864](https://github.com/graydonpleasants/geo-polygonize/commit/d3598640f3915f41499f151e12e0701d6bca656c))
* **core:** report excluded tiled endpoint components ([#1143](https://github.com/graydonpleasants/geo-polygonize/issues/1143)) ([b8b0dc1](https://github.com/graydonpleasants/geo-polygonize/commit/b8b0dc1b83d44e3418115ea411f581026fb4431e))
* **core:** report fixed-grid tiled components ([#1169](https://github.com/graydonpleasants/geo-polygonize/issues/1169)) ([2ef9c4d](https://github.com/graydonpleasants/geo-polygonize/commit/2ef9c4d417f2d96fbcfffcfa5628f70c52b3ba74))
* **core:** report pre-snap tiled components ([#1167](https://github.com/graydonpleasants/geo-polygonize/issues/1167)) ([5e1836f](https://github.com/graydonpleasants/geo-polygonize/commit/5e1836f47b64e1e5831ef700826b805ccd65f906))
* **core:** report tiled fallback decline reasons ([#1205](https://github.com/graydonpleasants/geo-polygonize/issues/1205)) ([19b92e0](https://github.com/graydonpleasants/geo-polygonize/commit/19b92e078dfb8e5947d540ede17b832638315c54))
* **core:** report tiled halo coverage evidence ([#1131](https://github.com/graydonpleasants/geo-polygonize/issues/1131)) ([0fbfad0](https://github.com/graydonpleasants/geo-polygonize/commit/0fbfad0b4f11918e15dfccf41b632ab0bd61a73a))
* **core:** report tiled input boundary evidence ([#1137](https://github.com/graydonpleasants/geo-polygonize/issues/1137)) ([ef9ba6f](https://github.com/graydonpleasants/geo-polygonize/commit/ef9ba6f350423c276efb48b8ffd297f625d49a16))
* **core:** report tiled source evidence completeness ([#1164](https://github.com/graydonpleasants/geo-polygonize/issues/1164)) ([c17d711](https://github.com/graydonpleasants/geo-polygonize/commit/c17d71147223f831e52aac6b9a1d4321ec0d9583))
* **core:** report unowned tiled faces ([#1222](https://github.com/graydonpleasants/geo-polygonize/issues/1222)) ([fd7919d](https://github.com/graydonpleasants/geo-polygonize/commit/fd7919d31ee69b0372fe0631641abaf457eb6f83))
* **core:** retain source chain boundaries ([#1250](https://github.com/graydonpleasants/geo-polygonize/issues/1250)) ([d45a65d](https://github.com/graydonpleasants/geo-polygonize/commit/d45a65d1d82db2b2f0bbab3193e505b5454977dd))
* **core:** retry unresolved tiles with bounded halos ([#1152](https://github.com/graydonpleasants/geo-polygonize/issues/1152)) ([898d11e](https://github.com/graydonpleasants/geo-polygonize/commit/898d11ee408fd48862f14463f36c4ea80763a22a))
* **core:** trace bounded tiled halo retries ([#1153](https://github.com/graydonpleasants/geo-polygonize/issues/1153)) ([e1db65f](https://github.com/graydonpleasants/geo-polygonize/commit/e1db65f8a087437497f33d22cdbce302438dc70a))
* **core:** trace canonical ordering ([#1024](https://github.com/graydonpleasants/geo-polygonize/issues/1024)) ([d295e66](https://github.com/graydonpleasants/geo-polygonize/commit/d295e66437a05960bc1967aeb95a9bf5aef7944e))
* **core:** trace certified candidates ([#1028](https://github.com/graydonpleasants/geo-polygonize/issues/1028)) ([cbaa41b](https://github.com/graydonpleasants/geo-polygonize/commit/cbaa41b0a0a6bbb3b6678a215a10ad19e56ed39b))
* **core:** trace certified hot pixels ([#1027](https://github.com/graydonpleasants/geo-polygonize/issues/1027)) ([c1be9f6](https://github.com/graydonpleasants/geo-polygonize/commit/c1be9f612b9dc1a5c800dcf33e44d54bfa0246db))
* **core:** trace certified split emission ([#1029](https://github.com/graydonpleasants/geo-polygonize/issues/1029)) ([6ca81c9](https://github.com/graydonpleasants/geo-polygonize/commit/6ca81c9cff348986b891bad8a79b023ecf58aee7))
* **core:** trace declined component fallback ([#1193](https://github.com/graydonpleasants/geo-polygonize/issues/1193)) ([d33216c](https://github.com/graydonpleasants/geo-polygonize/commit/d33216c45c8d578d5cd0c5cc2cc85675f5fe9b55))
* **core:** trace excluded tiled endpoint components ([#1146](https://github.com/graydonpleasants/geo-polygonize/issues/1146)) ([4d185cc](https://github.com/graydonpleasants/geo-polygonize/commit/4d185cc30a02223f464f485982262c008cc14284))
* **core:** trace excluded tiled segment components ([#1149](https://github.com/graydonpleasants/geo-polygonize/issues/1149)) ([ceceb71](https://github.com/graydonpleasants/geo-polygonize/commit/ceceb7176bde8bf143d15cd76bda563cc3081ee9))
* **core:** trace floating noding candidates ([#1030](https://github.com/graydonpleasants/geo-polygonize/issues/1030)) ([36729bb](https://github.com/graydonpleasants/geo-polygonize/commit/36729bb65a721e1cacf6b0f6ad3e976acad5c1f6))
* **core:** trace floating split emission ([#1034](https://github.com/graydonpleasants/geo-polygonize/issues/1034)) ([d7c75c7](https://github.com/graydonpleasants/geo-polygonize/commit/d7c75c77b0936e74d22d6a7164ca614e1f77bc02))
* **core:** trace global grid candidates ([#1033](https://github.com/graydonpleasants/geo-polygonize/issues/1033)) ([1140da8](https://github.com/graydonpleasants/geo-polygonize/commit/1140da8ec6a377736e0834377c028a8645f40422))
* **core:** trace graph topology ([#1020](https://github.com/graydonpleasants/geo-polygonize/issues/1020)) ([3adb74f](https://github.com/graydonpleasants/geo-polygonize/commit/3adb74f942774feecf874d37a9b991cf33bcdd6e))
* **core:** trace normalized input segments ([#1019](https://github.com/graydonpleasants/geo-polygonize/issues/1019)) ([b0ff52f](https://github.com/graydonpleasants/geo-polygonize/commit/b0ff52fb7499def04f3d09ed00fc02a2d91bb017))
* **core:** trace pruned topology ([#1021](https://github.com/graydonpleasants/geo-polygonize/issues/1021)) ([5b40a38](https://github.com/graydonpleasants/geo-polygonize/commit/5b40a3867aab88d23f57963950c727c0b5873777))
* **core:** trace ring containment ([#1023](https://github.com/graydonpleasants/geo-polygonize/issues/1023)) ([0ab66e7](https://github.com/graydonpleasants/geo-polygonize/commit/0ab66e728a20e3b214b710cfcf3a7590b83c7b0a))
* **core:** trace ring extraction ([#1022](https://github.com/graydonpleasants/geo-polygonize/issues/1022)) ([e08b8f2](https://github.com/graydonpleasants/geo-polygonize/commit/e08b8f203fd19ba7591b8225b85b9bc01d321a3d))
* **core:** trace snapped segments ([#1026](https://github.com/graydonpleasants/geo-polygonize/issues/1026)) ([cb325df](https://github.com/graydonpleasants/geo-polygonize/commit/cb325dfb999c8f668ae523d197c91f06e1330c31))
* **core:** trace tiled input boundary evidence ([#1140](https://github.com/graydonpleasants/geo-polygonize/issues/1140)) ([b0fa75b](https://github.com/graydonpleasants/geo-polygonize/commit/b0fa75bfe48e6894e48f8a7897a050b89361e02e))
* **core:** trace tiled owned-face boundaries ([#1141](https://github.com/graydonpleasants/geo-polygonize/issues/1141)) ([65532ae](https://github.com/graydonpleasants/geo-polygonize/commit/65532ae7ab69fe0aa634faf29e0079bfbd2405f1))
* **core:** trace tiled ownership ([#1025](https://github.com/graydonpleasants/geo-polygonize/issues/1025)) ([6b4e6e5](https://github.com/graydonpleasants/geo-polygonize/commit/6b4e6e5e06febd7c922d13a873b2ce9944cfd20a))
* **core:** trace uniform grid candidates ([#1032](https://github.com/graydonpleasants/geo-polygonize/issues/1032)) ([b9d5899](https://github.com/graydonpleasants/geo-polygonize/commit/b9d58992b85821f9ba20a896a6b378dde8a5bc53))
* **core:** trace uniform grid cells ([#1031](https://github.com/graydonpleasants/geo-polygonize/issues/1031)) ([89e0b89](https://github.com/graydonpleasants/geo-polygonize/commit/89e0b89606fb814b9cbca192b2752f07efa87d09))
* **core:** trace untiled tiled-coverage fallback ([#1157](https://github.com/graydonpleasants/geo-polygonize/issues/1157)) ([f92b102](https://github.com/graydonpleasants/geo-polygonize/commit/f92b102b25a7055cfa6e33334b1d9a835a270356))
* **core:** trace Z reconciliation decisions ([c5b1cdc](https://github.com/graydonpleasants/geo-polygonize/commit/c5b1cdcea2c639b283b361c5e70b89a1a423c430))
* **core:** validate observed tiled coverage ([#1138](https://github.com/graydonpleasants/geo-polygonize/issues/1138)) ([cfa9e5d](https://github.com/graydonpleasants/geo-polygonize/commit/cfa9e5d5082597a7456a5a7e1843787df7e89195))
* **wasm:** add playground GeoJSON ingestion ([#1049](https://github.com/graydonpleasants/geo-polygonize/issues/1049)) ([2958526](https://github.com/graydonpleasants/geo-polygonize/commit/2958526ba5e80c31c621a1b451c0726fe9e8eaf5))
* **wasm:** compare debugger profiles ([#1070](https://github.com/graydonpleasants/geo-polygonize/issues/1070)) ([6b77c5a](https://github.com/graydonpleasants/geo-polygonize/commit/6b77c5ad5231c194f846650fb72d8172554775b9))
* **wasm:** decode playground trace layers ([#1059](https://github.com/graydonpleasants/geo-polygonize/issues/1059)) ([fea7417](https://github.com/graydonpleasants/geo-polygonize/commit/fea7417b204461401625125be6e0a51e24854a56))
* **wasm:** draw playground linework ([#1050](https://github.com/graydonpleasants/geo-polygonize/issues/1050)) ([62524d9](https://github.com/graydonpleasants/geo-polygonize/commit/62524d9c38de0b0948dddf58f43b52a70edff36a))
* **wasm:** export debugger evidence bundles ([#1089](https://github.com/graydonpleasants/geo-polygonize/issues/1089)) ([7aff5c3](https://github.com/graydonpleasants/geo-polygonize/commit/7aff5c38f5155364659cf5ef65fa572958347873))
* **wasm:** export exact debugger fixtures ([fc71160](https://github.com/graydonpleasants/geo-polygonize/commit/fc711605670dbd1095de7e7032ae53c7f8b0f6c4))
* **wasm:** expose bounded topology traces ([#1054](https://github.com/graydonpleasants/geo-polygonize/issues/1054)) ([c75dd7b](https://github.com/graydonpleasants/geo-polygonize/commit/c75dd7b3a2068533b3b821338702753b53c5989a))
* **wasm:** expose canonical noding profiles ([#1051](https://github.com/graydonpleasants/geo-polygonize/issues/1051)) ([6f1ab70](https://github.com/graydonpleasants/geo-polygonize/commit/6f1ab70d97c7db18e41b6f3f1a8ff86bb9fb61f9))
* **wasm:** inspect playground trace provenance ([#1063](https://github.com/graydonpleasants/geo-polygonize/issues/1063)) ([9d837c0](https://github.com/graydonpleasants/geo-polygonize/commit/9d837c06ff97a63e85cc90fd953c2ec1cb03d0be))
* **wasm:** inspect Z reconciliation decisions ([#1077](https://github.com/graydonpleasants/geo-polygonize/issues/1077)) ([d698481](https://github.com/graydonpleasants/geo-polygonize/commit/d698481af0a5b935cfde5644f8ce126a364f0219))
* **wasm:** minimize debugger error differences ([#1096](https://github.com/graydonpleasants/geo-polygonize/issues/1096)) ([888c305](https://github.com/graydonpleasants/geo-polygonize/commit/888c305448c5ce6d226cf18844ce9e157072a40f))
* **wasm:** minimize debugger profile differences ([#1093](https://github.com/graydonpleasants/geo-polygonize/issues/1093)) ([07c4cce](https://github.com/graydonpleasants/geo-polygonize/commit/07c4cce5e5c5e70ebbfd9c34b20590fd04c65f91))
* **wasm:** render playground trace layers ([#1062](https://github.com/graydonpleasants/geo-polygonize/issues/1062)) ([e4c2531](https://github.com/graydonpleasants/geo-polygonize/commit/e4c2531324330f80414229879c423986fb956742))
* **wasm:** run playground traces in workers ([#1060](https://github.com/graydonpleasants/geo-polygonize/issues/1060)) ([5cbab11](https://github.com/graydonpleasants/geo-polygonize/commit/5cbab11d89687a6b6fccdf8bb02352991cbf755e))
* **wasm:** share playground repro URLs ([#1064](https://github.com/graydonpleasants/geo-polygonize/issues/1064)) ([6d55d82](https://github.com/graydonpleasants/geo-polygonize/commit/6d55d82b65ad4aff0c12c44a9b8847a33e165869))
* **wasm:** show debugger error witnesses ([#1072](https://github.com/graydonpleasants/geo-polygonize/issues/1072)) ([a0f6d05](https://github.com/graydonpleasants/geo-polygonize/commit/a0f6d05f2032d723edaf489026a5ec7fb76fe228))
* **wasm:** show debugger execution evidence ([#1078](https://github.com/graydonpleasants/geo-polygonize/issues/1078)) ([b490a4d](https://github.com/graydonpleasants/geo-polygonize/commit/b490a4dfd34007a9d08658080483b4012f114bd8))
* **wasm:** toggle retained topology layers ([#1052](https://github.com/graydonpleasants/geo-polygonize/issues/1052)) ([faa05b7](https://github.com/graydonpleasants/geo-polygonize/commit/faa05b76a94641c11d21eb015775588ab2b6f780))
* **wasm:** transport topology traces in workers ([#1055](https://github.com/graydonpleasants/geo-polygonize/issues/1055)) ([fa9d1eb](https://github.com/graydonpleasants/geo-polygonize/commit/fa9d1eb80cc53da773539e4cc8da378de4b9e702))
* **wasm:** type topology trace reports ([#1057](https://github.com/graydonpleasants/geo-polygonize/issues/1057)) ([c9f4422](https://github.com/graydonpleasants/geo-polygonize/commit/c9f4422d2e1179646a619c4e642dff6e8f419340))


### Bug Fixes

* **core:** bound certified validation ([#1256](https://github.com/graydonpleasants/geo-polygonize/issues/1256)) ([6012dee](https://github.com/graydonpleasants/geo-polygonize/commit/6012dee16b4ec5b4abd227a09a6efc4f864ea146))
* **core:** bound containment trace capture growth ([#1036](https://github.com/graydonpleasants/geo-polygonize/issues/1036)) ([ab132d3](https://github.com/graydonpleasants/geo-polygonize/commit/ab132d3b31a0f02bdc2572e4b8c9f2d789b91330))
* **core:** bound maximal-ring trace capture growth ([#1038](https://github.com/graydonpleasants/geo-polygonize/issues/1038)) ([56309fe](https://github.com/graydonpleasants/geo-polygonize/commit/56309feab7c71e1f171a5546e6764cddac694c24))
* **core:** bound noding trace capture growth ([#1035](https://github.com/graydonpleasants/geo-polygonize/issues/1035)) ([e09cbd8](https://github.com/graydonpleasants/geo-polygonize/commit/e09cbd81bf5efefa829eb6dbee88350630fefb8e))
* **core:** bound partial component cancellation ([#1204](https://github.com/graydonpleasants/geo-polygonize/issues/1204)) ([a44dc71](https://github.com/graydonpleasants/geo-polygonize/commit/a44dc718933879eff1c1608bec99bfd7ab04d18b))
* **core:** bound tiled call cardinality ([#1235](https://github.com/graydonpleasants/geo-polygonize/issues/1235)) ([ad7472e](https://github.com/graydonpleasants/geo-polygonize/commit/ad7472e5d7e2863e6eff3c8f3cab9181da4c4637))
* **core:** bound tiled component preflight work ([#1151](https://github.com/graydonpleasants/geo-polygonize/issues/1151)) ([f5338ce](https://github.com/graydonpleasants/geo-polygonize/commit/f5338cee395e1f82b62e3c4c38e41f509e4797eb))
* **core:** bound tiled merge output ([#1196](https://github.com/graydonpleasants/geo-polygonize/issues/1196)) ([6540e9e](https://github.com/graydonpleasants/geo-polygonize/commit/6540e9eb9be595e56d9a4142a5800b3fe2f16050))
* **core:** bound tiled trace capture growth ([#1037](https://github.com/graydonpleasants/geo-polygonize/issues/1037)) ([6e7c320](https://github.com/graydonpleasants/geo-polygonize/commit/6e7c320a192e9ff1ca278831260ece8f60054f03))
* **core:** detect excluded tiled segment components ([#1148](https://github.com/graydonpleasants/geo-polygonize/issues/1148)) ([3cd704b](https://github.com/graydonpleasants/geo-polygonize/commit/3cd704b8d767e826157ea5831b96676dc5486dcf))
* **core:** honor tiled filtering cancellation ([#1198](https://github.com/graydonpleasants/geo-polygonize/issues/1198)) ([e41c081](https://github.com/graydonpleasants/geo-polygonize/commit/e41c08120b04a1e00f21b7ae27df48681fbb07aa))
* **core:** merge duplicate tiled provenance ([#1236](https://github.com/graydonpleasants/geo-polygonize/issues/1236)) ([0875380](https://github.com/graydonpleasants/geo-polygonize/commit/08753804f82535c72d92bb640c8865386b7f78c0))
* **core:** normalize signed-zero graph identity ([#1264](https://github.com/graydonpleasants/geo-polygonize/issues/1264)) ([169a30a](https://github.com/graydonpleasants/geo-polygonize/commit/169a30acb8aa818b3070f2b3661da0694f535f9b))
* **core:** observe component fallback cancellation ([#1187](https://github.com/graydonpleasants/geo-polygonize/issues/1187)) ([ccc09ee](https://github.com/graydonpleasants/geo-polygonize/commit/ccc09ee0ab2c53559b5dac22f8b70a0908467461))
* **core:** observe tiled fallback merge cancellation ([#1197](https://github.com/graydonpleasants/geo-polygonize/issues/1197)) ([900f0a2](https://github.com/graydonpleasants/geo-polygonize/commit/900f0a22d592ae4ac32fb2889041efe83b1ce216))
* **core:** refresh arrangement degrees after edge deletion ([#1080](https://github.com/graydonpleasants/geo-polygonize/issues/1080)) ([80bc4b9](https://github.com/graydonpleasants/geo-polygonize/commit/80bc4b9ac653dc610034806e917ac34b20eaef14))
* **core:** remove duplicated tiled regression tests ([#1215](https://github.com/graydonpleasants/geo-polygonize/issues/1215)) ([5eadd54](https://github.com/graydonpleasants/geo-polygonize/commit/5eadd54c4a78c75819ec919c9ef3cd6875225f90))
* **core:** report partially observed components ([#1201](https://github.com/graydonpleasants/geo-polygonize/issues/1201)) ([2b3f5a1](https://github.com/graydonpleasants/geo-polygonize/commit/2b3f5a173f2f5a1fe66069ec6f5996c9cfac9801))
* **core:** reserve component scratch capacity correctly ([2d378d0](https://github.com/graydonpleasants/geo-polygonize/commit/2d378d0e0c8491b63b24c76e0bae5bef0d107208))
* **core:** reserve component scratch capacity correctly ([#1267](https://github.com/graydonpleasants/geo-polygonize/issues/1267)) ([d35f1d6](https://github.com/graydonpleasants/geo-polygonize/commit/d35f1d686dca8ea3e5dedadcc4e92d05d689be4c))
* **core:** validate excluded tiled endpoint components ([#1144](https://github.com/graydonpleasants/geo-polygonize/issues/1144)) ([8b27d9f](https://github.com/graydonpleasants/geo-polygonize/commit/8b27d9fb55f757da1992bb5145f2119e14d37e04))
* **github:** Implemented the CI/hardening pass: ([#1008](https://github.com/graydonpleasants/geo-polygonize/issues/1008)) ([dac3bd8](https://github.com/graydonpleasants/geo-polygonize/commit/dac3bd8f420135119fb4ed426f1bc2ca7d97b815))
* **github:** run required CI for stacked children ([#1066](https://github.com/graydonpleasants/geo-polygonize/issues/1066)) ([c4db618](https://github.com/graydonpleasants/geo-polygonize/commit/c4db6187d2eda8d56dd8e5b6a44743242899c267))
* **github:** skip automerge for native stacks ([#1058](https://github.com/graydonpleasants/geo-polygonize/issues/1058)) ([b4514f1](https://github.com/graydonpleasants/geo-polygonize/commit/b4514f12103472a4895bec7bfab29bc10e6b1944))
* **wasm:** parse noding iteration evidence ([#1087](https://github.com/graydonpleasants/geo-polygonize/issues/1087)) ([b6481fe](https://github.com/graydonpleasants/geo-polygonize/commit/b6481febb7def229168514430492c130a5b20f84))
* **wasm:** use full-page playground navigation ([#1263](https://github.com/graydonpleasants/geo-polygonize/issues/1263)) ([3f20d1e](https://github.com/graydonpleasants/geo-polygonize/commit/3f20d1ef167ed09e2c8de60ac8f53a65179683ab))


### Performance Improvements

* **core:** add certified fixed-precision benchmark lane ([#1005](https://github.com/graydonpleasants/geo-polygonize/issues/1005)) ([3828039](https://github.com/graydonpleasants/geo-polygonize/commit/38280399cee9fdfe289e3e73ba965b08f396de30))
* **core:** add certified JTS reference ([#1010](https://github.com/graydonpleasants/geo-polygonize/issues/1010)) ([581acc1](https://github.com/graydonpleasants/geo-polygonize/commit/581acc189ac5e000684a450e4ebd9457a70c1e65))
* **core:** add floating noding benchmark lane ([#1004](https://github.com/graydonpleasants/geo-polygonize/issues/1004)) ([3fc1ffd](https://github.com/graydonpleasants/geo-polygonize/commit/3fc1ffd00ecb5370d379e4993a1f5db5f865d812))
* **core:** benchmark component scaling ([#1255](https://github.com/graydonpleasants/geo-polygonize/issues/1255)) ([983c78b](https://github.com/graydonpleasants/geo-polygonize/commit/983c78b151f7b3e4f3c69f2d8701dd96d5939db3))
* **core:** define benchmark decision policy ([#1011](https://github.com/graydonpleasants/geo-polygonize/issues/1011)) ([2538e20](https://github.com/graydonpleasants/geo-polygonize/commit/2538e20367d4840ea236a1578ea5828aacc4d47c))
* **core:** define benchmark decision records ([#1013](https://github.com/graydonpleasants/geo-polygonize/issues/1013)) ([811f386](https://github.com/graydonpleasants/geo-polygonize/commit/811f386c8c422341742b3fdfa596322d1f3f8e88))
* **core:** gate benchmark artifact publication ([#1012](https://github.com/graydonpleasants/geo-polygonize/issues/1012)) ([ed6c39e](https://github.com/graydonpleasants/geo-polygonize/commit/ed6c39e801a7bf9f7e51c048cac23f0b89c647d6))
* **core:** publish correctness-gated benchmark records ([#1001](https://github.com/graydonpleasants/geo-polygonize/issues/1001)) ([5a0f2d3](https://github.com/graydonpleasants/geo-polygonize/commit/5a0f2d3f574892bb2c25bd031e1405e87c992bde))
* **core:** render benchmark trend evidence ([#1014](https://github.com/graydonpleasants/geo-polygonize/issues/1014)) ([1ccb17f](https://github.com/graydonpleasants/geo-polygonize/commit/1ccb17f2f5b1215ef886989953a178519872ace6))
* **core:** reuse component scratch ([#1252](https://github.com/graydonpleasants/geo-polygonize/issues/1252)) ([7162551](https://github.com/graydonpleasants/geo-polygonize/commit/7162551c14762eb2ea72e9f2e55d78fd8ad6716d))
* **core:** run correctness-gated already-noded benchmarks ([#1003](https://github.com/graydonpleasants/geo-polygonize/issues/1003)) ([713908a](https://github.com/graydonpleasants/geo-polygonize/commit/713908a79226bd9ea734476b193173d94d5b4db9))
* **core:** validate external topology references ([#1006](https://github.com/graydonpleasants/geo-polygonize/issues/1006)) ([044d687](https://github.com/graydonpleasants/geo-polygonize/commit/044d687aea0c6b20480fb2bcb5e5d6493c111057))
* **python:** separate polygonization output modes ([#1000](https://github.com/graydonpleasants/geo-polygonize/issues/1000)) ([928ee73](https://github.com/graydonpleasants/geo-polygonize/commit/928ee73caf340e3a435bcea2ce54f75b51028b94))

## [0.76.2](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.76.1...geo-polygonize-v0.76.2) (2026-07-24)


### Bug Fixes

* **core:** bound cancellation across preprocessing and grid construction ([#990](https://github.com/graydonpleasants/geo-polygonize/issues/990)) ([8820023](https://github.com/graydonpleasants/geo-polygonize/commit/88200231d4ba4f5451df347f35a08a79e937095d))
* **core:** enforce execution limits before allocation growth ([#989](https://github.com/graydonpleasants/geo-polygonize/issues/989)) ([30d16cc](https://github.com/graydonpleasants/geo-polygonize/commit/30d16cca4804c8658960a0adac3965277d0c9676))
* **core:** expose resource and cancellation FFI statuses ([#992](https://github.com/graydonpleasants/geo-polygonize/issues/992)) ([0b31ce8](https://github.com/graydonpleasants/geo-polygonize/commit/0b31ce87b40dc6982675a80b25894f1eb98c0507))
* **wasm:** preserve runtime selection in cancellable workers ([#998](https://github.com/graydonpleasants/geo-polygonize/issues/998)) ([924d9df](https://github.com/graydonpleasants/geo-polygonize/commit/924d9df064da9675146b4665df9a9b67c2d60602))

## [0.76.1](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.76.0...geo-polygonize-v0.76.1) (2026-07-24)


### Bug Fixes

* **core:** enforce noding limits inside candidate scans ([75aedde](https://github.com/graydonpleasants/geo-polygonize/commit/75aeddec627c7829635a7f321ae5676674f81889))
* **core:** enforce noding limits inside candidate scans ([#988](https://github.com/graydonpleasants/geo-polygonize/issues/988)) ([d214134](https://github.com/graydonpleasants/geo-polygonize/commit/d21413494feaba5a7b9d68d610672ab64b566709))
* **core:** preserve edge identity in topology fingerprints ([#986](https://github.com/graydonpleasants/geo-polygonize/issues/986)) ([7256ffe](https://github.com/graydonpleasants/geo-polygonize/commit/7256ffefef6615d77b6be938ece836bd9cf87fa9))

## [0.76.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.75.0...geo-polygonize-v0.76.0) (2026-07-23)


### Features

* **core:** poll coordinate restoration cancellation checks ([#983](https://github.com/graydonpleasants/geo-polygonize/issues/983)) ([ae789ba](https://github.com/graydonpleasants/geo-polygonize/commit/ae789ba6f2c4915e866be04a4a35595d1b601aff))
* **core:** poll noding validation cancellation checks ([#985](https://github.com/graydonpleasants/geo-polygonize/issues/985)) ([d2bbf34](https://github.com/graydonpleasants/geo-polygonize/commit/d2bbf347e370ce052dfc5cc9b124102c5198e097))

## [0.75.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.74.0...geo-polygonize-v0.75.0) (2026-07-23)


### Features

* **core:** poll input conversion cancellation checks ([#981](https://github.com/graydonpleasants/geo-polygonize/issues/981)) ([2a76621](https://github.com/graydonpleasants/geo-polygonize/commit/2a76621aa67b17a4e08a5419200eab6080a4bfaf))

## [0.74.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.73.0...geo-polygonize-v0.74.0) (2026-07-23)


### Features

* **core:** poll graph build cancellation checks ([#978](https://github.com/graydonpleasants/geo-polygonize/issues/978)) ([b2afa69](https://github.com/graydonpleasants/geo-polygonize/commit/b2afa6900270570441542ae4bc0d6489ed075f51))
* **core:** poll ingestion cancellation checks ([#979](https://github.com/graydonpleasants/geo-polygonize/issues/979)) ([8a9523e](https://github.com/graydonpleasants/geo-polygonize/commit/8a9523e1b66f4e5c6e1045b2796d03d30f14ab6c))

## [0.73.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.72.0...geo-polygonize-v0.73.0) (2026-07-23)


### Features

* **core:** poll canonicalization cancellation checks ([#976](https://github.com/graydonpleasants/geo-polygonize/issues/976)) ([f79a5c9](https://github.com/graydonpleasants/geo-polygonize/commit/f79a5c90d565cb32aeb2459564294d2b2a2587cd))

## [0.72.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.71.0...geo-polygonize-v0.72.0) (2026-07-23)


### Features

* **core:** poll polygon materialization cancellation checks ([#973](https://github.com/graydonpleasants/geo-polygonize/issues/973)) ([b6be944](https://github.com/graydonpleasants/geo-polygonize/commit/b6be944f84c81e834d89bf6382fba7b571d175df))

## [0.71.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.70.0...geo-polygonize-v0.71.0) (2026-07-23)


### Features

* **core:** poll ring traversal cancellation checks ([#971](https://github.com/graydonpleasants/geo-polygonize/issues/971)) ([afe14b3](https://github.com/graydonpleasants/geo-polygonize/commit/afe14b3be774af56a1541fb579136b41b57737a0))

## [0.70.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.69.0...geo-polygonize-v0.70.0) (2026-07-23)


### Features

* **core:** poll containment cancellation checks ([#970](https://github.com/graydonpleasants/geo-polygonize/issues/970)) ([1c9f282](https://github.com/graydonpleasants/geo-polygonize/commit/1c9f28205f2b6b2359f792b3acc60b83fa4b6a74))

## [0.69.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.68.0...geo-polygonize-v0.69.0) (2026-07-23)


### Features

* **core:** poll output cancellation checks ([#967](https://github.com/graydonpleasants/geo-polygonize/issues/967)) ([b7580da](https://github.com/graydonpleasants/geo-polygonize/commit/b7580daf61faeaa16f38e0f447a810596bef13e8))

## [0.68.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.67.0...geo-polygonize-v0.68.0) (2026-07-23)


### Features

* **core:** poll grid cancellation checkpoints ([#965](https://github.com/graydonpleasants/geo-polygonize/issues/965)) ([e4ea099](https://github.com/graydonpleasants/geo-polygonize/commit/e4ea0993d884fb231d9c82b68922e8725737f0e3))

## [0.67.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.66.0...geo-polygonize-v0.67.0) (2026-07-23)


### Features

* **core:** bound SIMD cancellation checks ([#963](https://github.com/graydonpleasants/geo-polygonize/issues/963)) ([9fafcaf](https://github.com/graydonpleasants/geo-polygonize/commit/9fafcafe8c08c03f7c5b577decc5b7e1114281b8))

## [0.66.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.65.0...geo-polygonize-v0.66.0) (2026-07-23)


### Features

* **core:** add native cancellation checkpoints ([#959](https://github.com/graydonpleasants/geo-polygonize/issues/959)) ([b1f97c5](https://github.com/graydonpleasants/geo-polygonize/commit/b1f97c527a4bada0c655bc71039d15df09fc7295))
* **python:** release GIL for cancellable runs ([#960](https://github.com/graydonpleasants/geo-polygonize/issues/960)) ([919c8aa](https://github.com/graydonpleasants/geo-polygonize/commit/919c8aafa0684b734c71a84d57eaef72e25461c4))
* **wasm:** add cancellable worker calls ([#961](https://github.com/graydonpleasants/geo-polygonize/issues/961)) ([c42adb6](https://github.com/graydonpleasants/geo-polygonize/commit/c42adb62ca525181f7184d0152c944869799fa63))

## [0.65.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.64.0...geo-polygonize-v0.65.0) (2026-07-22)


### Features

* **core:** cap result growth ([#956](https://github.com/graydonpleasants/geo-polygonize/issues/956)) ([b7850e4](https://github.com/graydonpleasants/geo-polygonize/commit/b7850e4e11bddbda58644230b8558f3a40c5fb07))

## [0.64.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.63.0...geo-polygonize-v0.64.0) (2026-07-22)


### Features

* **core:** cap graph growth ([#954](https://github.com/graydonpleasants/geo-polygonize/issues/954)) ([b1518bf](https://github.com/graydonpleasants/geo-polygonize/commit/b1518bfec53c04b4544a033841ffda049abdc709))

## [0.63.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.62.0...geo-polygonize-v0.63.0) (2026-07-22)


### Features

* **core:** cap split and noding passes ([#952](https://github.com/graydonpleasants/geo-polygonize/issues/952)) ([9cb75d6](https://github.com/graydonpleasants/geo-polygonize/commit/9cb75d6c6df89c1d083d2ca3b0eae78c27ae1344))

## [0.62.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.61.0...geo-polygonize-v0.62.0) (2026-07-22)


### Features

* **core:** bound noding work ([#950](https://github.com/graydonpleasants/geo-polygonize/issues/950)) ([ce6b1d3](https://github.com/graydonpleasants/geo-polygonize/commit/ce6b1d39eacae4ba33bd803fd1818338e05555c3))

## [0.61.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.60.0...geo-polygonize-v0.61.0) (2026-07-22)


### Features

* **core:** cap noded segment expansion ([#947](https://github.com/graydonpleasants/geo-polygonize/issues/947)) ([5cf386d](https://github.com/graydonpleasants/geo-polygonize/commit/5cf386d2288847f15373afa085053c0c60a97640))

## [0.60.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.59.0...geo-polygonize-v0.60.0) (2026-07-22)


### Features

* **core:** add input execution budgets ([#945](https://github.com/graydonpleasants/geo-polygonize/issues/945)) ([f7505e7](https://github.com/graydonpleasants/geo-polygonize/commit/f7505e79aaee5c016a99c8004bbdbff85d6b621e))

## [0.59.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.58.1...geo-polygonize-v0.59.0) (2026-07-22)


### Features

* **core:** add Arrow C ABI status contract ([#943](https://github.com/graydonpleasants/geo-polygonize/issues/943)) ([b28b776](https://github.com/graydonpleasants/geo-polygonize/commit/b28b776e1992d55f03c035b22caf541e1274d1a6))

## [0.58.1](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.58.0...geo-polygonize-v0.58.1) (2026-07-22)


### Bug Fixes

* **wasm:** use canonical report in playground ([d8fa39d](https://github.com/graydonpleasants/geo-polygonize/commit/d8fa39da6a227fbea830482fb67dc687b32ee8e0))
* **wasm:** use canonical report in playground ([#939](https://github.com/graydonpleasants/geo-polygonize/issues/939)) ([e9bac6e](https://github.com/graydonpleasants/geo-polygonize/commit/e9bac6e90bb7885bcfd23fa9528266a9aeb48399))

## [0.58.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.57.0...geo-polygonize-v0.58.0) (2026-07-22)


### Features

* **wasm:** add adapter conformance fixture ([#935](https://github.com/graydonpleasants/geo-polygonize/issues/935)) ([433b85e](https://github.com/graydonpleasants/geo-polygonize/commit/433b85e4611c0b332968c0cf541d8ad4df9f4db9))

## [0.57.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.56.0...geo-polygonize-v0.57.0) (2026-07-22)


### Features

* **python:** expose normalized adapter errors ([#933](https://github.com/graydonpleasants/geo-polygonize/issues/933)) ([dc13645](https://github.com/graydonpleasants/geo-polygonize/commit/dc136456b25a0b0a05cd73150f1d10ee7f154d16))

## [0.56.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.55.0...geo-polygonize-v0.56.0) (2026-07-22)


### Features

* **wasm:** generate topology report types ([#931](https://github.com/graydonpleasants/geo-polygonize/issues/931)) ([4e32e62](https://github.com/graydonpleasants/geo-polygonize/commit/4e32e6243f5bc0ff8fd4b20d7b6a386965507282))

## [0.55.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.54.0...geo-polygonize-v0.55.0) (2026-07-22)


### Features

* **wasm:** add full topology report API ([#929](https://github.com/graydonpleasants/geo-polygonize/issues/929)) ([a01e197](https://github.com/graydonpleasants/geo-polygonize/commit/a01e197d570c7f8122d9201374b8ba2b77e1ea4d))

## [0.54.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.53.0...geo-polygonize-v0.54.0) (2026-07-22)


### Features

* **wasm:** expose GeoJSON conformance fingerprint ([#926](https://github.com/graydonpleasants/geo-polygonize/issues/926)) ([24c785b](https://github.com/graydonpleasants/geo-polygonize/commit/24c785b025d8fdfb69f06298e5805e136ea3b0f7))

## [0.53.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.52.0...geo-polygonize-v0.53.0) (2026-07-22)


### Features

* **python:** expose conformance fingerprints ([#924](https://github.com/graydonpleasants/geo-polygonize/issues/924)) ([381a01f](https://github.com/graydonpleasants/geo-polygonize/commit/381a01fb95310fdfccd30507c2e57afe02708be0))

## [0.52.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.51.2...geo-polygonize-v0.52.0) (2026-07-22)


### Features

* **core:** add topology conformance fingerprint ([#922](https://github.com/graydonpleasants/geo-polygonize/issues/922)) ([cf6a519](https://github.com/graydonpleasants/geo-polygonize/commit/cf6a51982034c62a08b6283a277243cd95e24f8e))

## [0.51.2](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.51.1...geo-polygonize-v0.51.2) (2026-07-22)


### Bug Fixes

* **github:** allow BSD-2-Clause dependencies ([#904](https://github.com/graydonpleasants/geo-polygonize/issues/904)) ([f5b9e07](https://github.com/graydonpleasants/geo-polygonize/commit/f5b9e075795ec1e546b04829255b88bb0a5ff37e))

## [0.51.1](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.51.0...geo-polygonize-v0.51.1) (2026-07-22)


### Bug Fixes

* **core:** classify cut edges before ring extraction ([#902](https://github.com/graydonpleasants/geo-polygonize/issues/902)) ([43c309a](https://github.com/graydonpleasants/geo-polygonize/commit/43c309af2435ed8ad532f8dcfeb61afc14c544a0))

## [0.51.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.50.0...geo-polygonize-v0.51.0) (2026-07-22)


### Features

* **core:** extract FlatGeoBuf adapter crate ([#900](https://github.com/graydonpleasants/geo-polygonize/issues/900)) ([8745276](https://github.com/graydonpleasants/geo-polygonize/commit/8745276414a6eae0eb2bdce1ba639d2497a3b9b8))

## [0.50.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.49.0...geo-polygonize-v0.50.0) (2026-07-22)


### Features

* **python:** extract extension crate ([#898](https://github.com/graydonpleasants/geo-polygonize/issues/898)) ([11b99cb](https://github.com/graydonpleasants/geo-polygonize/commit/11b99cb9dc4d0f5c0232a29545f2f687965ad92e))

## [0.49.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.48.0...geo-polygonize-v0.49.0) (2026-07-22)


### Features

* **core:** extract Arrow adapter crate ([#896](https://github.com/graydonpleasants/geo-polygonize/issues/896)) ([a7fb4d4](https://github.com/graydonpleasants/geo-polygonize/commit/a7fb4d4f72c0165ca9be5052e7a111a7ab1240dd))

## [0.48.0](https://github.com/graydonpleasants/geo-polygonize/compare/geo-polygonize-v0.47.1...geo-polygonize-v0.48.0) (2026-07-21)


### Features

* **core:** make Arrow adapters optional ([#894](https://github.com/graydonpleasants/geo-polygonize/issues/894)) ([d487f63](https://github.com/graydonpleasants/geo-polygonize/commit/d487f63811cdd0fc6a15952e1bfdb60a89de2686))

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
