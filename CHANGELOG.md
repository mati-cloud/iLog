# [0.2.0](https://github.com/mati-cloud/iLog/compare/v0.1.0...v0.2.0) (2026-01-02)


### Bug Fixes

* **agent:** resolve compilation errors in ilog-agent ([f2f7a02](https://github.com/mati-cloud/iLog/commit/f2f7a02574fbf68e9482c6b540871f4c453cdc0c))
* **backend:** add slug field to services ([1a52a70](https://github.com/mati-cloud/iLog/commit/1a52a7003e8603d0e06852aa4245efb7f717aac6))
* **backend:** correct registration token URL path to /login ([d1cc0af](https://github.com/mati-cloud/iLog/commit/d1cc0af9679732710657afcf9c240648eef43ae2))
* **backend:** correct string repeat syntax in info! macro ([5aaeb7c](https://github.com/mati-cloud/iLog/commit/5aaeb7cf6ab555aeb8de4575ea1ef69441ca1617))
* **backend:** make user table foreign keys conditional ([7162421](https://github.com/mati-cloud/iLog/commit/71624217551144eb2fe56540aaa13526ea208d54))
* **backend:** rename user_tokens to agents table in SQL queries ([27b27fb](https://github.com/mati-cloud/iLog/commit/27b27fbca63734cfe42da057cf109c2d33d3bb64))
* **ci:** add --tls=[secure] flag to all dind services ([251fd77](https://github.com/mati-cloud/iLog/commit/251fd77590d6e5ee42bf287b0b8a79b3751c7ecb))
* **ci:** configure Git authentication for semantic-release ([4ee8987](https://github.com/mati-cloud/iLog/commit/4ee898787105952654b49e4ef14e5e97edc6fec4))
* **ci:** disable TLS for docker-in-docker to resolve certificate errors ([08032f2](https://github.com/mati-cloud/iLog/commit/08032f26c00dec506163ebca16d7e42d8c684f8d))
* **ci:** disable TLS to match runner configuration ([f527842](https://github.com/mati-cloud/iLog/commit/f5278426d96c58072b7252678104ded101b7da87))
* **ci:** remove (unused) curl package and fix proper docker image name ([cf8a112](https://github.com/mati-cloud/iLog/commit/cf8a112af431f45669ae9f9d3027a6d5dfe20839))
* **ci:** remove repositoryUrl from semantic-release config ([7fc5758](https://github.com/mati-cloud/iLog/commit/7fc5758a29825ec6f73e74f9807ce090e5b60c86))
* **ci:** resolve buildx race condition in manifest creation ([350273c](https://github.com/mati-cloud/iLog/commit/350273c6c1fcecb9890507005476c4f9c6eb2c34))
* **ci:** revert TLS configuration that broke Docker connectivity ([4d219d6](https://github.com/mati-cloud/iLog/commit/4d219d6e7cfc1248a3dc2041d3a76189d9ee76f0))
* **ci:** set DOCKER_TLS_CERTDIR on dind service containers ([af80b36](https://github.com/mati-cloud/iLog/commit/af80b3614024689826557bdc9922e54b790c5dac))
* **ci:** set DOCKER_TLS_CERTDIR to match runner configuration ([dd8b63c](https://github.com/mati-cloud/iLog/commit/dd8b63cbf2ec99ab924c9d0a51352e34fbcfe06f))
* **ci:** use default buildx builder instead of docker-container driver ([6f801b4](https://github.com/mati-cloud/iLog/commit/6f801b479c8d8b60355966ad3e03cc2c4563c08b))
* **ci:** use GitLab as primary repository for semantic-release ([a90f041](https://github.com/mati-cloud/iLog/commit/a90f041cb423b8400fe04db97889466fc4c1f7d4))
* **ci:** use HTTPS URL for GitLab repository in semantic-release ([fcaed42](https://github.com/mati-cloud/iLog/commit/fcaed4212964f5e9b54b6907e308f15762241d1e))
* **ci:** use proper filename for bun's lockfile when building frontend image ([f8870e7](https://github.com/mati-cloud/iLog/commit/f8870e7cc9558c66e80a89d74a7c4546b239f184))
* **docker:** disable incremental compilation for sccache compatibility ([3e49024](https://github.com/mati-cloud/iLog/commit/3e49024786d99774fe4967e1ba1e9c56ab88df3a))
* **docker:** set CARGO_INCREMENTAL=0 in Dockerfile for sccache ([af36c9d](https://github.com/mati-cloud/iLog/commit/af36c9d8ad7c80628c69d17199eaaea0b23a5489))
* **frontend:** add source_type field to service creation ([653e012](https://github.com/mati-cloud/iLog/commit/653e012142d195841c710d52433aa03602382cc0))
* **frontend:** grant write permissions to public directory for runtime config ([bb69408](https://github.com/mati-cloud/iLog/commit/bb694082439a7fd4401eb7c2384e0cf536feb28e))
* **frontend:** use correct frontend URL for auth client ([a7736dc](https://github.com/mati-cloud/iLog/commit/a7736dc26961c2b92d8a767f6e9ee3dd29d5c00d))
* **frontend:** use runtime config for all API and WebSocket URLs ([20dcd45](https://github.com/mati-cloud/iLog/commit/20dcd4578ff2c2570f1f8f8b901fccbfb605d5f1))
* **frontend:** wrap login page in Suspense boundary ([5edb6a3](https://github.com/mati-cloud/iLog/commit/5edb6a3b03aaffcb699faca974b3a18db01cef0c))
* remove initial admin user via signup-token ([5402d0b](https://github.com/mati-cloud/iLog/commit/5402d0b2d50120748ea8a4fa6262010caf7c18d2))
* remove symlink ([3b4ee2f](https://github.com/mati-cloud/iLog/commit/3b4ee2f5344c580490b6d67bf1cfc19973e2279f))


### Features

* **auth:** add secure initial admin registration with one-time tokens ([5042c0c](https://github.com/mati-cloud/iLog/commit/5042c0c77bfc22fedf490da4b2b8effa214653fd))
* **backend:** add Better Auth compatibility migration ([e005324](https://github.com/mati-cloud/iLog/commit/e0053247e541a72de5a1e5d69066d7eac1d91748))
* **ci:** add direct download URLs to release notes ([3be492d](https://github.com/mati-cloud/iLog/commit/3be492d51663b2383341766082805449e200566f))
* **ci:** add GitHub Releases for ilog-agent binaries ([e649cf7](https://github.com/mati-cloud/iLog/commit/e649cf73e8735f622fb1e72a5a3d4cdf1db22794))
* **ci:** add semantic-release for automatic versioning ([eeb7330](https://github.com/mati-cloud/iLog/commit/eeb7330fdd801e8e4b8747e4b43580f123532568))
* cleanup ([e6c09c0](https://github.com/mati-cloud/iLog/commit/e6c09c09afeff73fcdef1fb9463bba756d8e5909))
* **frontend:** add automatic database migrations on container startup ([36302d4](https://github.com/mati-cloud/iLog/commit/36302d4233db8e16b047e96ed4d0ab4c080c3cad))
* **frontend:** add JWT plugin migration and include migrations in Docker image ([ae4d6c0](https://github.com/mati-cloud/iLog/commit/ae4d6c0ece4a6d068f2fa4bac025b637204aed86))
* **frontend:** implement runtime configuration for self-hosted deployments ([4cbc005](https://github.com/mati-cloud/iLog/commit/4cbc0057b4853e77643ed17c399e69e592cc39dd))
* **migrations:** keep better-auth migrations present within frontend ([2e5bae6](https://github.com/mati-cloud/iLog/commit/2e5bae6d20e4124feea2d7a59b8b038b8e1eab0e))
* **ui:** hide sidebar when not authenticated and improve sign-in UX ([9dae6cd](https://github.com/mati-cloud/iLog/commit/9dae6cdf3294565c22ae1fe9185f3d2205bc7cce))


### Performance Improvements

* **docker:** optimize build speed with cargo-chef and registry cache ([f919a8a](https://github.com/mati-cloud/iLog/commit/f919a8ae1371b159958e7de1d46443bd18c2d608))
* **rust:** add sccache, mold linker, and cargo optimizations ([9ae2e3f](https://github.com/mati-cloud/iLog/commit/9ae2e3f6bfec540c7badbd160c8a9c3bcb8aab90))


### Reverts

* **ci:** remove TLS-disabling configuration to use runner's TLS setup ([e62683a](https://github.com/mati-cloud/iLog/commit/e62683af7cad833d176fa5cd1e4d19841a60e6c1))
