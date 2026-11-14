# Changelog

## [0.14.0](https://github.com/intigraph/tambo/compare/v0.13.4...v0.14.0) (2026-11-13)


### Features

* **skills:** publish native pendants and sync pinned profiles ([#276](https://github.com/intigraph/tambo/issues/276)) ([b5f668e](https://github.com/intigraph/tambo/commit/b5f668e5eb739f904d96568bf9807929d80ecc2c))

## [0.13.4](https://github.com/intigraph/tambo/compare/v0.13.3...v0.13.4) (2026-11-11)


### Bug Fixes

* compute install checksums in-process, not via sha256sum on PATH ([#269](https://github.com/intigraph/tambo/issues/269)) ([53476e6](https://github.com/intigraph/tambo/commit/53476e666cffbcf1f81036e843d72ba2d99dd82e))

## [0.13.3](https://github.com/intigraph/tambo/compare/v0.13.2...v0.13.3) (2026-11-10)


### Bug Fixes

* **verify:** give isolated episodes an explicit namespace ([#267](https://github.com/intigraph/tambo/issues/267)) ([75a9afc](https://github.com/intigraph/tambo/commit/75a9afc93313c0842b9a7e1dbb5ae22536304f9d))

## [0.13.2](https://github.com/intigraph/tambo/compare/v0.13.1...v0.13.2) (2026-11-09)


### Bug Fixes

* **camayoc:** pin verified v0.1.6 source release ([#263](https://github.com/intigraph/tambo/issues/263)) ([e076a83](https://github.com/intigraph/tambo/commit/e076a8309a07b29d317429df3a91083adb1e98fe))
* **hooks:** ingest Codex turns through Stop stdin ([#265](https://github.com/intigraph/tambo/issues/265)) ([8360bea](https://github.com/intigraph/tambo/commit/8360bea000a01e9ecc60f1bb3cd1e42633baa0a9))

## [0.13.1](https://github.com/intigraph/tambo/compare/v0.13.0...v0.13.1) (2026-11-07)


### Bug Fixes

* **census:** isolate authenticated release metadata reads ([#257](https://github.com/intigraph/tambo/issues/257)) ([1fb4b50](https://github.com/intigraph/tambo/commit/1fb4b5031a4867c939ddd391eb588fbc185faad3))

## [0.13.0](https://github.com/intigraph/tambo/compare/v0.12.1...v0.13.0) (2026-11-06)


### Features

* **install:** prepare Linux arm64 archive selection ([#258](https://github.com/intigraph/tambo/issues/258)) ([78484a1](https://github.com/intigraph/tambo/commit/78484a131cc458c74f94a1b01675fede99b6819d))

## [0.12.1](https://github.com/intigraph/tambo/compare/v0.12.0...v0.12.1) (2026-11-06)


### Bug Fixes

* **census:** rebind documented installers to current releases ([#255](https://github.com/intigraph/tambo/issues/255)) ([3fb2812](https://github.com/intigraph/tambo/commit/3fb28127542219462c0d52fc7a9107209ff3a85a))

## [0.12.0](https://github.com/intigraph/tambo/compare/v0.11.3...v0.12.0) (2026-11-06)


### Features

* prepare bounded phase-A and own-UID enrollment packets ([#254](https://github.com/intigraph/tambo/issues/254)) ([0c2bb4b](https://github.com/intigraph/tambo/commit/0c2bb4b0cfb9b3c9bf9b3ec7e5eabcfe2a97a3f2))


### Bug Fixes

* **qualification:** require explicit Homebrew formula trust ([#248](https://github.com/intigraph/tambo/issues/248)) ([62b2727](https://github.com/intigraph/tambo/commit/62b2727c14450ccce147e5360663a1e81ea856f2))

## [0.11.3](https://github.com/intigraph/tambo/compare/v0.11.2...v0.11.3) (2026-11-06)


### Bug Fixes

* **update:** preserve exact Quipu runtime aliases through rollback ([#160](https://github.com/intigraph/tambo/issues/160)) ([17206d2](https://github.com/intigraph/tambo/commit/17206d26206b6b1737935405921d022d325c3da4))

## [0.11.2](https://github.com/intigraph/tambo/compare/v0.11.1...v0.11.2) (2026-11-05)


### Bug Fixes

* **ci:** fetch pinned Quipu verifier from canonical repository ([#243](https://github.com/intigraph/tambo/issues/243)) ([8ffeb60](https://github.com/intigraph/tambo/commit/8ffeb603e4bf503976dcc6b93f95dfa0a71c164d))
* **macos:** distinguish implicit controller groups from explicit grants ([#245](https://github.com/intigraph/tambo/issues/245)) ([194bad9](https://github.com/intigraph/tambo/commit/194bad93da758a90eb97fb1b38db7b27ba6af284))

## [0.11.1](https://github.com/intigraph/tambo/compare/v0.11.0...v0.11.1) (2026-11-05)


### Bug Fixes

* **macos:** accept signed system account inventory IDs ([#241](https://github.com/intigraph/tambo/issues/241)) ([509f576](https://github.com/intigraph/tambo/commit/509f576f71f925235a9313b79805dd68c5e738ef))
* **quickstart:** retain newer Bobbin under skip-install ([#244](https://github.com/intigraph/tambo/issues/244)) ([8ca86f7](https://github.com/intigraph/tambo/commit/8ca86f7138fc149f7508dc1fbe9cabe703fadc00))
* **runtime:** ship CA-capable macOS job runtime inputs ([#240](https://github.com/intigraph/tambo/issues/240)) ([6236480](https://github.com/intigraph/tambo/commit/62364807aa6597e11f476a1cf5eb1a590716382a))

## [0.11.0](https://github.com/intigraph/tambo/compare/v0.10.1...v0.11.0) (2026-11-05)


### Features

* add macOS controller packet and pinned job runtime ([#238](https://github.com/intigraph/tambo/issues/238)) ([ebb0a83](https://github.com/intigraph/tambo/commit/ebb0a83b6b130fd88d455d29765dc3e6bc42d55b))

## [0.10.1](https://github.com/intigraph/tambo/compare/v0.10.0...v0.10.1) (2026-11-05)


### Bug Fixes

* preserve repository pendant carriers and pack aliases ([#235](https://github.com/intigraph/tambo/issues/235)) ([46bf1e8](https://github.com/intigraph/tambo/commit/46bf1e8975d206bd13a6f5e8df70f405590d7191))

## [0.10.0](https://github.com/intigraph/tambo/compare/v0.9.0...v0.10.0) (2026-11-05)


### Features

* **hooks:** install and verify the query-first hooks for Claude Code and Codex ([#227](https://github.com/intigraph/tambo/issues/227)) ([82f710e](https://github.com/intigraph/tambo/commit/82f710e4723a56f5f1160bc9412c4ee91689fa71))


### Bug Fixes

* require pinned embedding providers in repository shares ([#233](https://github.com/intigraph/tambo/issues/233)) ([0461437](https://github.com/intigraph/tambo/commit/04614379596614e4aa5acc39576bce3b2356a6b8))

## [0.9.0](https://github.com/intigraph/tambo/compare/v0.8.0...v0.9.0) (2026-11-04)


### Features

* **packs:** name stack-map entities under w3id.org/intigraph/tambo ([#221](https://github.com/intigraph/tambo/issues/221)) ([76f63a2](https://github.com/intigraph/tambo/commit/76f63a2e00b01d92c30c99c86a59f02dc5160572))


### Bug Fixes

* bootstrap Seeds instead of the legacy tracker ([#225](https://github.com/intigraph/tambo/issues/225)) ([3c6688f](https://github.com/intigraph/tambo/commit/3c6688ff6b3b117185a5bafc8dd2776f80be663f))
* **compat:** drop the vision line [#224](https://github.com/intigraph/tambo/issues/224) removed from the legacy-name ledger ([#226](https://github.com/intigraph/tambo/issues/226)) ([a3b6765](https://github.com/intigraph/tambo/commit/a3b676560c0ba54e3b390ca0ede995c6d1c6c79a))
* **macos:** accept native dscl properties and honor packet directory modes ([#222](https://github.com/intigraph/tambo/issues/222)) ([517f6c7](https://github.com/intigraph/tambo/commit/517f6c7c6b0522be33792e3818da90a2dd7c7b34))

## [0.8.0](https://github.com/intigraph/tambo/compare/v0.7.1...v0.8.0) (2026-11-02)


### Features

* **jobs:** prepare a pinned Mac account operator packet ([#212](https://github.com/intigraph/tambo/issues/212)) ([b196d44](https://github.com/intigraph/tambo/commit/b196d441c2cbe29ebbeb110212663bd2ca3dcb42))


### Bug Fixes

* **compat:** classify the eight drifted legacy-name files ([#217](https://github.com/intigraph/tambo/issues/217)) ([36eb5a5](https://github.com/intigraph/tambo/commit/36eb5a5366310fc850d7dde2c8cbe4f16bc169d9))
* **demos:** keep the prompt colour on every row of a wrapped command ([#218](https://github.com/intigraph/tambo/issues/218)) ([c4406c9](https://github.com/intigraph/tambo/commit/c4406c9a94f874059812a2c4ee63a448ca273986))
* **install:** install Camayoc from intigraph/camayoc ([#215](https://github.com/intigraph/tambo/issues/215)) ([d1e0f48](https://github.com/intigraph/tambo/commit/d1e0f48feae89605ea7c01df9289fc31e414e15b))

## [0.7.1](https://github.com/intigraph/tambo/compare/v0.7.0...v0.7.1) (2026-10-30)


### Bug Fixes

* **tests:** keep brew PATH fixture aligned with seeds pin (aegis-ko21) ([#209](https://github.com/intigraph/tambo/issues/209)) ([b40e46a](https://github.com/intigraph/tambo/commit/b40e46a8202062151c9102a13c6ee5681b7b1d87))

## [0.7.0](https://github.com/intigraph/tambo/compare/v0.6.4...v0.7.0) (2026-10-28)


### Features

* **install:** wire camayoc's memory-file redirect hook into Claude and Codex, with verify (aegis-gshu.1) ([#202](https://github.com/intigraph/tambo/issues/202)) ([610cbe6](https://github.com/intigraph/tambo/commit/610cbe69eba94109b94a22513d121f5682f70c65))
* **memory:** plan/apply/restore/verify for the agent-memory setup (aegis-gshu.9) ([#203](https://github.com/intigraph/tambo/issues/203)) ([76ee7c8](https://github.com/intigraph/tambo/commit/76ee7c88a02fbd9c61414587563caa339706699a))
* **prime:** rikuy brief site reachability in tambo prime (aegis-sp21) ([#201](https://github.com/intigraph/tambo/issues/201)) ([3ce2863](https://github.com/intigraph/tambo/commit/3ce2863e5f7790686e6e97c125c520b6f53642b8))
* **prime:** stack prime contract and tambo prime ([#200](https://github.com/intigraph/tambo/issues/200)) ([4cbc9b3](https://github.com/intigraph/tambo/commit/4cbc9b3bd46065270efe816eba8e9ec0719b142c))


### Bug Fixes

* reconcile the reviewed product-name inventory ledger (aegis-i0p2) ([#198](https://github.com/intigraph/tambo/issues/198)) ([3973eda](https://github.com/intigraph/tambo/commit/3973edaa8c8458cfb6d21e26f5c5e513e143b394))

## [0.6.4](https://github.com/intigraph/tambo/compare/v0.6.3...v0.6.4) (2026-10-27)


### Bug Fixes

* **update-self:** follow GitHub renames and read intigraph/tambo ([#196](https://github.com/intigraph/tambo/issues/196)) ([df6454e](https://github.com/intigraph/tambo/commit/df6454e980c9a702978e9d707ffae5eef702025a))

## [0.6.3](https://github.com/intigraph/tambo/compare/v0.6.2...v0.6.3) (2026-10-25)


### Bug Fixes

* **stack:** pin Bobbin 0.30.0, Yupana 0.16.2, Desire Path v0.10.0 and Shuttle 0.6.1 ([#193](https://github.com/intigraph/tambo/issues/193)) ([39c134d](https://github.com/intigraph/tambo/commit/39c134df8e8eb279a70aa33ca0fc7a522f221c17))

## [0.6.2](https://github.com/intigraph/tambo/compare/v0.6.1...v0.6.2) (2026-10-24)


### Bug Fixes

* **hooks:** refresh the shipped Yupana bundle to 0.15.4 ([#190](https://github.com/intigraph/tambo/issues/190)) ([bcf91fc](https://github.com/intigraph/tambo/commit/bcf91fc36c75417b9fb758e081bb8bad9d3fbcf4))

## [0.6.1](https://github.com/intigraph/tambo/compare/v0.6.0...v0.6.1) (2026-10-23)


### Bug Fixes

* **release:** record pendant admission and verify only when admitted ([#186](https://github.com/intigraph/tambo/issues/186)) ([45b2ffb](https://github.com/intigraph/tambo/commit/45b2ffb4d8802d9ec7310d03b6269195ad94db7b))

## [0.6.0](https://github.com/intigraph/tambo/compare/v0.5.0...v0.6.0) (2026-10-23)


### Features

* add repository knowledge pendant source ([#172](https://github.com/intigraph/tambo/issues/172)) ([6db2109](https://github.com/intigraph/tambo/commit/6db2109095785981ced864106c14a0a6949636cd))


### Bug Fixes

* complete Tambo branding and preserve compatibility contracts ([#173](https://github.com/intigraph/tambo/issues/173)) ([032919c](https://github.com/intigraph/tambo/commit/032919cca58cb656da69cf09c46e6af57e8a0482))
* **knowledge:** stop witnessing Cargo.toml, which every release rewrites ([#185](https://github.com/intigraph/tambo/issues/185)) ([154fbb0](https://github.com/intigraph/tambo/commit/154fbb05c6b90948bcf69d2df6e619ad92667123))
* **release:** use canonical Tambo owner and retain repository guard ([#183](https://github.com/intigraph/tambo/issues/183)) ([60744cf](https://github.com/intigraph/tambo/commit/60744cf5fbfa5eba724b38818c8b1a7a12b780c3))
* **share:** publish and verify the repository share as intigraph/tambo ([#184](https://github.com/intigraph/tambo/issues/184)) ([8ed2f2d](https://github.com/intigraph/tambo/commit/8ed2f2d8b5fa8cdba2b3ee6d911877d20a460cc3))

## [0.5.0](https://github.com/scbrown/tambo/compare/v0.4.0...v0.5.0) (2026-10-21)


### Features

* **members:** declare explicit compatible identity alternatives ([#174](https://github.com/scbrown/tambo/issues/174)) ([7ca1260](https://github.com/scbrown/tambo/commit/7ca1260652c384653e271fa0b67abed51b19e330))

## [0.4.0](https://github.com/scbrown/tambo/compare/v0.3.7...v0.4.0) (2026-10-19)


### Features

* **pendants:** select native monorepo project shares by checkout ([#167](https://github.com/scbrown/tambo/issues/167)) ([fe969dd](https://github.com/scbrown/tambo/commit/fe969dd7e9439dd6f50c9d9484f41f72cd7d4f77))
* **pendants:** sync repository packs with opt-in hooks ([#165](https://github.com/scbrown/tambo/issues/165)) ([6054a00](https://github.com/scbrown/tambo/commit/6054a00fbeeedc9541c9049d40b8d2f19ca73b14))
* **trust:** expose endpoint decisions for federation consumers ([#168](https://github.com/scbrown/tambo/issues/168)) ([70199b4](https://github.com/scbrown/tambo/commit/70199b472793ff94e1639c3d9c1b405ac1b3396d))

## [0.3.7](https://github.com/scbrown/tambo/compare/v0.3.6...v0.3.7) (2026-10-17)


### Bug Fixes

* **packages:** verify manifest members through package launchers ([#161](https://github.com/scbrown/tambo/issues/161)) ([ed418d9](https://github.com/scbrown/tambo/commit/ed418d9fd3e8c74ee8c4754967e060ff8262a587))

## [0.3.6](https://github.com/scbrown/tambo/compare/v0.3.5...v0.3.6) (2026-10-17)


### Bug Fixes

* **homebrew:** verify the package Quipu pair on PATH ([#158](https://github.com/scbrown/tambo/issues/158)) ([2cbf4c4](https://github.com/scbrown/tambo/commit/2cbf4c44283dea6dc80c2caa412b10eef88e2fb9))

## [0.3.5](https://github.com/scbrown/tambo/compare/v0.3.4...v0.3.5) (2026-10-16)


### Bug Fixes

* **homebrew:** follow stable updates beyond embedded release pins ([d96e4e8](https://github.com/scbrown/tambo/commit/d96e4e86d3dad744bda0214b93e6c5b4e9df6eba))

## [0.3.4](https://github.com/scbrown/tambo/compare/v0.3.3...v0.3.4) (2026-10-16)


### Bug Fixes

* **doctor:** a rejected credential no longer hides the namespace check (aegis-vgu) ([#149](https://github.com/scbrown/tambo/issues/149)) ([00fc5cc](https://github.com/scbrown/tambo/commit/00fc5cc8b72b509965bbb57188ed0d1a569de1c0))
