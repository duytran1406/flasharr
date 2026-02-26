# Changelog

## [2.1.0](https://github.com/duytran1406/flasharr/compare/v2.0.0...v2.1.0) (2026-02-26)


### Features

* Add SABnzbd and Newznab API integration for Sonarr/Radarr ([cd83090](https://github.com/duytran1406/flasharr/commit/cd8309022f5a4b78801072afebc8263dc043fdf0))
* **collection:** add library status + Smart Search/Add buttons to collection page ([dd66a1b](https://github.com/duytran1406/flasharr/commit/dd66a1bde963f88a485d62246d5543fc1fe444fd))
* Complete Smart Search Bridge with Response Converter ([c61cd17](https://github.com/duytran1406/flasharr/commit/c61cd17d14e825cc8fa96497776c6bd53d46477b))
* implement Add to Library button for TV shows and movies ([9248aa1](https://github.com/duytran1406/flasharr/commit/9248aa1398a3e9ec3e2750486bac8cc4f4285557))
* Implement Smart Search Bridge for Newznab Indexer ([975f2c6](https://github.com/duytran1406/flasharr/commit/975f2c6b027a0190337f259666cbdf6b4c96fd28))
* intelligent error handling in download engine ([77b574d](https://github.com/duytran1406/flasharr/commit/77b574d3abe3ca505be187a020377b62ccb67b49))
* overhaul intro animation - Terminal Breach disappear, glitch-fade entry, continuous particles ([5f59daf](https://github.com/duytran1406/flasharr/commit/5f59daf3ba26d8bbeb7360e53a5dd9315b277987))
* parallel multi-alias Fshare search for all title categories ([f93e5cd](https://github.com/duytran1406/flasharr/commit/f93e5cd9a7af466c8b4015ae14b4610d31b18627))
* restructure docs, add auto semver, rewrite README ([8f8174d](https://github.com/duytran1406/flasharr/commit/8f8174d5c9e7013a122288bf67c8dc7b7bd10ad1))
* **search:** 10-column grid + infinite scroll, replace pagination ([d5f03d3](https://github.com/duytran1406/flasharr/commit/d5f03d30cb9b2933099f71f5c880d89355f6aac6))
* **snowball:** two-tier search — Tier1 basic-per-group, Tier2 targeted per-gap ([97c960e](https://github.com/duytran1406/flasharr/commit/97c960e7993d2b2ceb337bdc83ba57e587416957))


### Bug Fixes

* /api/media/{id}/downloads always returns 200; add library IDs to response ([dc5cf23](https://github.com/duytran1406/flasharr/commit/dc5cf23d86680c5b1617be3ecf6e72a789e63c87))
* active queue blink + library sync on delete ([811f8cf](https://github.com/duytran1406/flasharr/commit/811f8cff4b46276d2ee798438b0381f806f4a6b7))
* **backend:** forward Radarr collection field in RadarrMovie struct ([55f53d9](https://github.com/duytran1406/flasharr/commit/55f53d97e506314b3321963335d38545dfd6eee1))
* batch sync metrics, account rank detection, download sync improvements ([20d0157](https://github.com/duytran1406/flasharr/commit/20d0157d5418ae95ae81118812de401e3de9ad18))
* **ci:** use static sha- prefix for Docker tag traceability ([0777f8f](https://github.com/duytran1406/flasharr/commit/0777f8fd34b2044b5b44b0bfbfa535f9d9aa4e2f))
* **collection:** auto-add to Radarr when Smart Search is triggered on missing movie ([84f0206](https://github.com/duytran1406/flasharr/commit/84f020652282000b7639697055404a4d4e0316f3))
* **collection:** instant library status update on Add to Library ([aaebd9c](https://github.com/duytran1406/flasharr/commit/aaebd9c63428f3ffb05cd355fc5b8e1577aeaa15))
* dashboard library section blank — history API shape mismatch ([f5ef91d](https://github.com/duytran1406/flasharr/commit/f5ef91d7ef3acf317e26d938389c6f4f44bc591d))
* **dashboard:** bottom crop — use height:100% instead of calc(100vh...) ([0908e68](https://github.com/duytran1406/flasharr/commit/0908e68e86592735c61a75bdfc1e6b35d612f6b3))
* **dashboard:** downloading items cleared on first TASK_BATCH_UPDATE ([b50b4c5](https://github.com/duytran1406/flasharr/commit/b50b4c562fecf0de1e430fe2a80325d8322d9c1b))
* **dashboard:** shrink Recently Added cards, fix Active Queue batch visibility ([5eee05f](https://github.com/duytran1406/flasharr/commit/5eee05fed8c0b17f682d851d5c8f1cdb509a5024))
* **downloads:** prevent 422 "missing field year" and null tmdb_id ([6f08e70](https://github.com/duytran1406/flasharr/commit/6f08e7003ff7a613b0f92f3d0b431d28d91f16cf))
* **downloads:** prevent item accumulation across page navigation ([f0ac8f7](https://github.com/duytran1406/flasharr/commit/f0ac8f7ac48d006ef819094461c6a70e907a9281))
* **downloads:** reduce page size from 20 to 16 to eliminate scrolling ([bade3a1](https://github.com/duytran1406/flasharr/commit/bade3a1ced868bdcf70a103abda36291de672d94))
* extract quality metadata from original API filename, not clean Sonarr name ([867ab38](https://github.com/duytran1406/flasharr/commit/867ab380e55d60a19942cbb05ee75f6112e9773c))
* **favicon:** scale to 192x192 for high-DPI sharpness, add apple-touch-icon ([01e62f4](https://github.com/duytran1406/flasharr/commit/01e62f46d2ade2cc125e84326fd5d1645087821b))
* **library/discover:** effect_orphan, infinite scroll, Collections tab, favicon ([5e8fdfd](https://github.com/duytran1406/flasharr/commit/5e8fdfdf452a3d005040e489c8ee0ddfc4a9b099))
* quality parser false-positive TS detection and misleading SD fallback ([3708828](https://github.com/duytran1406/flasharr/commit/370882858af07a751079b759105506082540cbbb))
* reduce search_cache TTL from 1h to 10min ([c981acc](https://github.com/duytran1406/flasharr/commit/c981acc10e697f6b9ac6fcd602bcb43395939f4f))
* reset retry_count on manual resume to give fresh retry budget ([9e72fd8](https://github.com/duytran1406/flasharr/commit/9e72fd835c75f8120aa01400dcc56492bcdca550))
* **sabnzbd:** skip empty/unknown destinations in history response ([d579a51](https://github.com/duytran1406/flasharr/commit/d579a512369e8fa17c77f453b62927edc6002c92))
* **search:** filter out non-media files from all search results ([14fff86](https://github.com/duytran1406/flasharr/commit/14fff86e48c05c8fb7466634bd3c6b390b3f236e))
* **search:** revert grid to 5 per row ([c6c8f25](https://github.com/duytran1406/flasharr/commit/c6c8f25fae78a861f9a9c0834bd2525b4950e028))
* **settings:** validate folder source URL before triggering cache sync ([abcd185](https://github.com/duytran1406/flasharr/commit/abcd1858af53bc52b680eacdbb135ce62b3dcf30))
* Smart Grab 422 error - convert priority to string ([a696192](https://github.com/duytran1406/flasharr/commit/a6961927fbd4dbc576360bdd79242e387083f36f))
* **smart-grab:** show correct aired/available episode coverage ([189ca80](https://github.com/duytran1406/flasharr/commit/189ca8074321566edea42b480e61f9e0ae4125a5))
* **snowball:** treat each filename pattern as a distinct release group ([47f9e4b](https://github.com/duytran1406/flasharr/commit/47f9e4b428afd1360f97bad21b443de6b3cac9f3))
* StatusChecker health fetch errors on navigation + reduce console noise ([fdab8ff](https://github.com/duytran1406/flasharr/commit/fdab8ff27c61d0fb68b93dc963c95d28152e095f))
* **tmdb:** cache fetch_tmdb_title to prevent rate-limit on batch grab ([8837bd3](https://github.com/duytran1406/flasharr/commit/8837bd35894c3b1c98a091357fba2889aba4d22e))
* tv overview fallback to original language when English overview is empty ([184f1e8](https://github.com/duytran1406/flasharr/commit/184f1e8f09604a52e4c8ee4b31ed9ed84a8baece))
* **ui:** cinematic poster cards + Discover badge text fix ([465f872](https://github.com/duytran1406/flasharr/commit/465f87278828c5768b71a0aa2c8fe1e5215b8905))


### Performance Improvements

* **smart-search:** parallelize Snowball Tier 1 & Tier 2 ([c7ca577](https://github.com/duytran1406/flasharr/commit/c7ca577e473098a3d1071e5d34a33d4764b47235))
