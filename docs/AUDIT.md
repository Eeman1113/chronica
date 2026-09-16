# AUDIT.md — full audit of the Chronica prototype (`index.html`, 8,452 lines, commit c253cd5)

Method: six parallel full-read passes over contiguous ranges (1–1080, 1050–2330, 2300–3580,
3550–5010, 4990–5960, 5900–8452), plus first-hand verification of every cheat site named in the
rebuild directive (see DECISIONS D-003). `index.html` ≡ `chronica.html` (byte-identical).

Layout: §1 systems & functions · §2 entities & state · §3 events · §4 relationships ·
§5 persistence · §6 cheats/approximations/schedules/prunes with emergent replacements ·
§7 determinism findings · §8 bugs found · §9 rule conflicts.

---

## §1 — Systems and complete function inventory

The file is one page: CSS/DOM shell (1–203), then script blocks: core+worldgen+vegetation
(203–1309), worm mind (1310–1454), sea/beast/relics/eras/books (1455–1947), pro-mode social
(1948–2185), economy (2186–2635), society/knowledge (2636–3273), simulation tick (3274–~5795),
renderer+UI+persistence+boot (~5795–8452). Cadence legend: **T**=per-tick(day), **M**=monthly,
**Q**=quarterly, **Y**=yearly, **E**=on-event, **I**=init-only, **R**=render/UI-only, **H**=helper.

### Core utilities & RNG (203–265)
`$`(210,R) `clamp`(211,H) `lerp`(212,H) `hashStr`(213,H FNV-1a seed hash) `mulberry32`(214,H PRNG
factory) global `R`(215) `rnd`(216) `ri`(217) `rf`(218) `pick`(219) `chance`(220) `shuffle`(221)
`makeNoise`(224,I value-noise/fbm, own stream) `Heap`(246,+push 247/pop 249, A* heap)
`b64FromBuf`(256)/`bufFromB64`(259, save codecs) `bd`(270,I biome registrar).

### Names & cultures (355–443)
`makePhon`(358,I) `wordFrom`(368,H — defaults to world stream R!) `makeCultures`(380,I — stream
`seed^0x5EEDFACE`) `personName`(395,E) `surName`(401,E) `settName`(402,E) `facCoreName`(404,E)
`worldName`(405,E) `religionName`(407,E). Dead data: `OLD_CULTURES_DISABLED`(413–443).

### Grid, calendar, events, people (460–530)
`idx`(460) `inb`(461) `isWater`(462) `passCost`(463) `passable`(473) `curYear`(475)
`curSeason`(476) `dateStr`(477) — all H. `logEvent`(480,E — see §3) `rebuildEvMap`(512,E)
`getEv`(513,H) `toast`(514,R) `newPerson`(517,E) `fullName`(526,H) `ageOf`(527,H) `getP`(528,H
living→souls fallback) `bio`(529,E capped) `addDist`(530,E renown, notable at ≥6).

### World generation (533–652) — I
`generateWorld`(533): elevation/moisture fbm noise, radial continent mask, rivers as downhill
walkers from sampled high tiles (558–591), biome paint (593–613), lake flood-fill (615–622),
resources + ore quota top-up (624–652).

### Pathfinding (657–711)
`findPath`(657,E A*, maxIter 3000) `greedyPath`(693,E noisy greedy ≤40 steps).

### Settlements & factions (713–970)
`siteScore`(713,H) `foundSettlement`(742,E) `buildStruct`(768,E) `expandSettlement`(812,E)
`settPop`(834,H scan) `settPopAll`(835,H) `settSize`(836,H from popC) `newFaction`(839,E)
`facTitle`(846,H rank by settlement count) `leaderTitle`(853,H) `facPop`(857,H scan)
`relKey`(858)/`getRel`(859)/`setRel`(860,H diplomacy scalar) `seedCivilizations`(863,I)
`seedAnimals`(921,I 0.7×cap rejection sampling) `recomputeTerritory`(945,Q BFS paint)
`newSim`(974,I).

### Vegetation (993–1308) — aggregate per-cell densities, no individual plants
`vegAlloc`(1007,I) `vegSpFor`(1018,I) `vegInit`(1030,I) `vegTemp`(1058,H) `vegSuit`(1064,H)
`vegDaily`(1075,T — each cell every 21 days via `VEG_STRIDE` stripes: moisture, litter/soil,
germination, growth, stress, dieback, gene selection, seeding, blight, succession, biome flips)
`nearRiver1`(1187,H) `vegMonthly`(1193,M — 420 random-tile dispersal + ≤40 grazer carriers, farm
blight) `vegYearly`(1255,Y census/extinction) `vegSer`(1278)/`vegMoistFromBiome`(1284)/
`vegDeser`(1292, save/load).

### Worm mind (1310–1454) — real C. elegans connectome per animal (brains mode)
`wormInit`(1327,I decode 302-neuron connectome from 38.7KB base64, CSR wiring, GABA signs)
`wormNewBrain`(1384,E per-animal LIF state) `wormInject`(1389,H ±30% jitter) `mutateGenes`(1390,E
4 heritable genes ±5%) `wormStep`(1396,T ~2 RNG draws per neuron per step — dominant RNG consumer)
`wormLocomotion`(1441,T rates→{dx,dy,mode}).

### Sea (1455–1703)
`seaPassable`(1458,H) `coastWaterNear`(1459,H) `landNearWater`(1468,H) `findSeaPath`(1473,E ocean
A*) `settHarbor`(1499,H daily-memo) `newShip`(1509,E) `crewAshore`(1510,E) `sinkShip`(1524,E)
`updateShips`(1537,T storm-sink roll, fish income, 2 tiles/step) `shipArrive`(1566,E trade/settle/
war effects) `monthlySea`(1633,M fishing boats with real crew) `trySeaMigration`(1658,E 50 random
site samples) `armyToSea`(1691,E).

### Great beast (1705–1777) — singleton
`beastName`(1707,E) `spawnGreatBeast`(1712,E — vestigial per comment at 1925) `updateBeast`(1726,T
roam/stalk/kill-random-resident/teleport-back) `fightBeast`(1760,E stat-weighted dice).

### Relics, eras, books, timelapse (1779–1946)
`createRelic`(1781,E) `passRelics`(1795,E inheritance) `lootRelics`(1814,E conquest)
`classifyYears`(1830,H) `updateEras`(1851,Y 10-year blocks) `makeBook`(1873,E random 5–7 event
sample) `snapTerritory`(1892,Y RLE, cap 420) `decodeSnap`(1911,R) `yearlyExtras`(1922,Y).

### Pro-mode social (1948–2184)
`befriend`(1958,E cap 6) `proQuarterlySocial`(1964,Q random-pool bonds) `grievanceOf`(1982,H)
`proMonthlyPlots`(1994,M plot seed/recruit/uprising — mechanistic) `realmNeeds`(2051,H ore scan of
first tile only) `proYearlyWars`(2068,Y grievance-driven wars, runs in EVERY mode per 5686)
`noteAnimalKill`(2108,E legend promotion thresholds) `proOnBereavement`(2121,E grief; pro-only
grief-prophets at 2125) `proMonthlyStress`(2139,M breakdowns) `proYearlyRelics`(2169,Y ≤1
sword/year world-wide, cap 9).

### Economy (2186–2635)
`invCap`(2205,H) `initEconSt`(2207,I lazy) `initEconFac`(2223,I) `foodTotal`(2225,H)
`toolMult`(2226,H) `econDaily`(2228,T production from profession head-counts, consumption
0.85/person, spoilage) `econMonthly`(2270,M price ← demand/supply blend) `monthlyNeed`(2284,H)
`econPartnersOf`(2295,H omniscient 40-tile fallback) `tradeMonthly`(2305,M best-arbitrage caravan/
ship dispatch) `launchTradeFlagClear`(2361,E) `monOf`(2367,H lazy) `facCurrency`(2373,H)
`curPenalty`(2377,H) `econArrive`(2385,E goods handover, coin/commodity/barter) `moneyYearly`(2448,Y
money-stage state machine none→barter→commodity→coin) `fiscalMonthly`(2535,M tax/wages)
`maybeDebase`(2557,E wartime debasement) `moneyPanel`(2588,R — but see §6.8) `invPanel`(2607,R —
see §6.8) `econSerSt`(2618)/`econDeserSt`(2623, save/load).

### Knowledge, memory, bonds, society (2636–3273)
`KNOW`(2641, 9 crafts + prereqs) `knows`(2652,H) `teach`(2653,E) `settHasSkill`(2661,H day-cache)
`rebuildFacKn`(2669,H)/`facKnows`(2675,H)/`facKnownList`(2679,R) `settGeo`(2687,H 90d cache)
`residentsByHome`(2709,H) `topOfProf`(2718,H) `discRipe`(2728,H chance gate on all discoveries)
`discReqOk`(2729,H) `discFire`(2733,E) `knowledgeMonthly`(2739,M ≤1 discovery/town/month;
apprenticeships 720 days) `knowledgeYearly`(2853,Y crafts lost with last knower) `remember`(2869,E
memory cap 8/16) `witnessSettlement`(2878,E) `memoryYearly`(2882,Y decay ×0.97) `inheritMemories`
(2890,E told-tales at 60%) `grievance2`(2900,H — overrides grievanceOf at 3271) `edgesOf`(2914,H)
`edgeAdd`(2920,E typed bonds cap 10/16 — overrides befriend at 3272) `closeCompanions`(2936,H)
`onViolentDeath`(2946,E friend-slain memories) `householdMonthly`(2955,M famine lending, debt →
grievance or friendship) `stewardsQuarterly`(3004,Q loyalty drift, dice-free secession)
`capKnown`(3047,H cap 16) `hearNews`(3055,E imperfect information records) `knownPartners`(3078,H)
`knownFacPop`(3095,H heard population ×3, else truth×rf(0.6,1.5)) `succession2`(3103,E claims/
support/score; rival ≥80% → civil war) `cultureOf`(3183,H) `mutatePhon`(3188,E language drift)
`contactYearly`(3194,Y assimilation at 40y, culture forking) `onDeathInherit`(3251,E wealth split).

### Simulation tick (3274–3550)
`tick`(3280,T master loop: weather, veg, fires, corpses, animal grid, settlements, people,
animals, armies, caravans, migrants, ships; `%30===7`→monthly, `%90===3`→quarterly,
`%360===180`→yearly, `%90===44`→territory) `updateWeather`(3302,T spells 3–9 days)
`igniteWildfire`(3318,E 30 random probes) `updateFires`(3331,T real-fuel spread, hard stop >350
tiles) `burnBuilding`(3384,E 25% random-resident kill) `updateSettlements`(3404,T popC census,
farm formula, famine/plague counters, starvation) `randomResident`(3497,H uniform pick)
`updatePeople`(3504,T mortality roll, age transitions, movement) `stepAlong`(3541,H)
`woodCap`(3547,H) `homeTile`(3548,H).

### Town infrastructure & trips (3561–4106)
`maybeBuildPen`(3561,E dice-free) `buryPerson`(3595,E graves cap 240) `graveyardYearly`(3619,Y
remembrance-based weathering) `tunnelQuarterly`(3656,Q dice-free siting by real detour ratio)
`tunnelMonthly`(3730,M bore, 2% collapse kills random crew member) `planTownWall`(3781,E gates at
road crossings, else random tile) `wallsMonthly`(3809,M masons lay real courses)
`leadsMonthly`(3850,M rope from wood+grass) `huntsMonthly`(3865,M culls from threat ledger, real
bait corpse) `homesMonthly`(3931,M housing by family wealth; homeless) `maybeStartTrip`(3979,T 12%
work-trip roll; **LOD stop at 4001**) `resolveMission`(4042,T hunter encounter dice)
`bonus`(4100,E).

### Death (4107–4185)
`killPerson`(4107,E soul record, **souls pruned >22000 at 4133**, succession trigger)
`succession`(4149,E → succession2) `heirLoc`(4157,H) `splitFaction`(4159,E rebellion mechanics)
`checkFactionDeath`(4173,E).

### Animals (4187–4963)
`updateAnimals`(4187,T lifespan reroll, starvation/drowning counters, mind dispatch, **fortnightly
world-pulse reproduction 4263–4284**) `rebuildAnimalGrid`(4291,T 12-cell spatial hash)
`nearAnimal`(4301,H) `nearestPredAnimal`(4317,H) `nearestPreyAnimal`(4320,H) `spawnCorpse`(4325,E
cap 160) `corpseStage`(4333,H) `updateCorpses`(4334,T rot→fertility) `nearestCorpse`(4351,H)
`feedFromCorpse`(4360,E) `isDrinkable`(4377,H) `findWaterNear`(4378,H memoized)
`stepAnimal`(4388,T movement primitive honoring fences/walls/gates/territory)
`salmonUpdate`(4411,T/2 utility mind, real headwater spawning) `crocUpdate`(4486,T/2 ambush mind)
`instinctAnimalUpdate`(4564,T needs→utility argmax; predation, herds, man-eating, town watch,
grazing real veg) `wormAnimalUpdate`(4885,T neural mind; think rationed by speed, **selected
animal exempt at 4941**) `nearestAnimal`(4960,H) `removeAnimal`(4963,E).

### War (4966–5225)
`updateArmies`(4966,T supply attrition ~3% desertion) `marchStep`(5008,T 1–2 tiles, wears roads)
`fieldBattle`(5027,E casualties rf(0.06,0.14), rout chance(0.25)) `siegeTick`(5046,T formula
defenders, fall after ri(10,20), raze coin-flip <14 pop) `armyCasualties`(5075,E kills real
members from roster end) `retreatArmy`(5085,E) `disbandArmy`(5086,E) `conquerSettlement`(5098,E
flat 12% deaths, 60% grudge rolls) `razeSettlement`(5121,E 70% flee rolls) `nearestSettName`
(5142,H) `areAtWar`(5150,H) `warOf`(5151,H) `declareWar`(5152,E) `musterArmies`(5163,E real draft
+ grain logistics gate) `nearestEnemySett`(5201,H) `maybePeace`(5210,E score≥24 ∥ dur>4y ∥
**chance(0.15)**).

### Movement of groups & epidemics (5227–5293)
`updateCaravans`(5227,T religion 30% / plague 35% spread on arrival) `updateMigrants`(5259,T)
`startPlague`(5287,E).

### Calendar aggregates (5295–5795)
`monthly`(5295,M construction ladder, gate-closing roll, sanitation, births gated by grain-days +
housing, migration trigger, all *Monthly systems, pro plots/stress) `monthlyDomestic`(5377,M
taming chance(0.5) at proximity+lead) `tryMigration`(5394,E 40 random sites)
`assignProfessions`(5429,Q quota reassignment) `quarterly`(5478,Q professions, tunnels, social,
stewards, scarcity caches, marriage chance(0.4)) `yearly`(5527,Y threat decay, pen upkeep, scar/
lake/canal healing rolls, diplomacy drift, border raids, proYearlyWars (all modes), re-muster,
**prophet founding (non-pro) 5700–5715**, religion spread rolls, knowledge/memory/contact/money
yearlies, species re-immigration, drought roll, one-plague-per-year outbreak, stats push)
`facPopCached`(5794,H yearly-stale).

### Renderer, UI, persistence, boot (5799–8452) — R unless noted
`resize`(5801) `glyphTile`(5811) `ensureAtlas`(5827) `hexA`(5832) `viewRange`(5836)
`renderTerrain`(5842) `drawGlyphAt`(5928) `smooth`(5935 — writes rx/ry onto sim entities)
`render`(5944) `renderWeather`(6101) `followEntity`(6129) `renderMinimap`(6144); Observatory:
`obsFmt`(6193) `obsPct`(6194) `obsNum`(6195) `obsBack`(6199) `obsSearchBox`(6202) `obsMenuRow`
(6205) `obsAgeY`(6209) `obsLivePeople`(6210) `obsLiveSetts`(6211) `obsLiveFacs`(6212)
`obsWarPairs`(6213) `obsEvline`(6226) `obsSearch`(6231) `obsWorldMenu`(6265) `obsOverview`(6274)
`obsGeography`(6342) `obsResources`(6372) `obsLifeMenu`(6408) `obsPeople`(6419) `obsPeopleList`
(6455) `obsFamilies`(6480) `obsAnimals`(6499) `obsSpecies`(6517) `obsCarrion`(6568) `obsHealth`
(6585) `obsSocietyMenu`(6604) `obsSettlements`(6617) `obsEconomy`(6645) `obsTechnology`(6693)
`obsReligion`(6714) `obsMilitary`(6727) `obsCulture`(6752) `obsNatureMenu`(6778) `obsVegetation`
(6789) `obsWater`(6837) `obsFire`(6856) `obsWildlife`(6873) `obsFoodweb`(6889) `renderObservatory`
(6971); `ui`(7032 tick/toast/refreshSoon/setTab/renderPanel) `esc`(7078) `selLabel`(7080)
`trailPush`(7096) `trailHtml`(7104) `jumpTo`(7113) `L`(7114) `renderInspect`(7117) `renderTile`
(7132) `renderPerson`(7243) `renderCorpse`(7331) `renderRelic`(7357) `famNode`(7369)
`showFamilyTree`(7378) `showBook`(7385) `renderSett`(7390) `renderFac`(7441) `renderShip`(7481)
`renderArmy`(7521) `renderAnimal`(7537) `renderReligion`(7617) `renderRealms`(7630)
`renderChronicle`(7646) `renderStats`(7672) `drawCharts`(7693) `drawLine`(7704); handlers
7729–7980 incl. lesion tool (7731, **mutates sim**), `showEventCard`(7801), `pickAt`(7882),
`toggleAvatar`(7956); `hwMaxSpeedIx`(7982) `buildSpeedButtons`(7989) `setSpeed`(8004)
`syncSpeedUI`(8005) `togglePause`(8007) `showModal`(8013) `openTimelapse`(8031) `drawTL`(8053)
`tlSetPlay`(8070) `showHelp`(8083); `saveString`(8133,E) `loadString`(8177,E) `alertBox`(8246,R)
`bootWorld`(8249,I) `frame`(8299,R tick driver) `drawWormRaster`(8327,R — **mutates sim at 8333**)
`ensureWormAnatBg`(8363) `drawWormAnat`(8402); autosave interval (8428), hidden-tab ticker (8433),
startup (8446–8451).

---

## §2 — Entity types and state (complete field lists)

**world** (538–543, 1007–1017): `seedStr seed W H name`; per-cell typed arrays `elev moist
moistBase biome river road(usage counter) building resource variant cut(scar codes 1 cleared/2
lakebed/3 burn/4 ruin/5 overgrazed/6 tunnel) settleId terrId`; vegetation layer `veg(biomass)
vsp(one species/cell) litter soilN seedB(bitmask seed bank) vgen(one drought gene/cell) blight`;
mode flags `brains pro best`.

**sim** (975–981): `day nextId people(Map) souls(Map) animals[] armies[] caravans[] migrants[]
settlements[] factions[] religions[] relations{} events[] evSeq evMap fires{} ships[] relics[]
eras[] books[] snaps[] beast corpses[] mon{} currencies[] tunnels[] yb yd yfever lastYB lastYD
weather{type,t}(global) drought(global bool) stats{yr,pop,wild,setts,fac,veg,mon,evoPrey,evoPred}
pathQueue[] fireEvId`.

**person** (518–522 + later): `id nm sur sex born fac home prof x y rx ry path pi mission hunger
dist amb brav pie cha spouse fa mo kids[] bio[] notable inArmy dead` + `skills[] _app _stu mem[]
frs[](typed bonds) wealth loyal sea tun hst hx hy homeless carry grudge lost stress plot relics[]
pie-driven prof transitions`. Personality is 4 uniform scalars; no health/nutrition/knowledge-
beliefs beyond `skills`; hunger only during trips.

**soul** (4130): frozen `id nm sur sex born died prof fac fa mo spouse kids notable dist
bio(notables only) dead` — lossy snapshot.

**settlement** (746–749 + later): `id nm x y fac founded tiles[] houses farms market temple keep
walls stores rad canals[] hasCanal stock{wood,stone,leads} religion famine plague plagueT
plagueDead razed log[] lastCaravan conqDay why popC emptyDays hasHorse lvTxt lvLost pen gates{}
gy wallPlan wallAt walled hunt huntCd baitId threat{} homeless steward inv(Float32×9)
price(Float32×9) _sup _dem trade{vol,fr,gcount,partners} wealth known{} contact drift cul _asY`
+ caches `_car _skD/_skC _geoD/_geo _rt _harborD/_harbor _cropH _gameMod _forestMod _hasOre
_salty _blY` + throttles `famEv plagueEv lastFamLog lastFireLog lastPlotLog lastLakeLog _stvY
_gateShame`.

**faction** (840–841 + later): `id nm color culture capital leader relig rel{} wars{} dead dyn
treasury unpaid known{} crownDay _kn _knPrev _lastDeb _noMarch _pop`.

**animal** (935–937 + later): `id sp x y rx ry age path pi hp genes(Float32×4: wariness appetite
boldness herd — or worm sensory gains) gen hun thr fat dom act sense think sickD _starv _drown
_cause _bite _tnm wx wy wdir noWater _viaGate manKills kills legend spawned run brain _seed`.

**army** (5197): `id fac x y rx ry members[] str target home path pi state(march|siege|return) gen
fought siegeT sup dead`. **caravan** (2335): `id from to fac x y rx ry path pi relig plague
cargo{g,qty} dead`. **ship** (1509): `id kind(fisher|trade|settle|war) fac from to x y rx ry path
pi members[] cargo ret relig plague dx dy why target gen age dead`. **migrant band** (5427): `id
fac from members[] x y rx ry path pi dx dy dead why causeEvId`.

**religion** (5708): `id nm color(hardcoded) founder founded`. **currency** (2496): `id nm issuer
metal purity born dead orphan`. **money state** (2370): `stage medium curId since`. **relic**
(1788): `id type nm holder sett log[]`. **corpse** (4325): `id sp x y d rot mass cause bait
baitSt`. **tunnel** (3700s): `id fac sponsor to line[] prog need camp workers[] state began`.
**pen** (3588): `{x0 y0 x1 y1 gx gy cond}`. **graveyard** (3596): `{tiles[] graves[{p,d,w}] old}`.
**war** (5154): `{since score ev}`. **era** (1853): `{from to type nm}`. **book** (1886): `{id
title fac d text}`. **snap** (1908): `{y d(RLE)}`. **memory** (2871): `{d k imp fac p?}`. **bond**
(2930): `{id t(friend|comrade|cred) s amt? bad?}`. **beast singleton** (1722): `{id nm born kills
evId slain slayer grown}`. **culture** (392): `{id ph settSuf fem adj}`. **worm brain** (1384):
`{v(F32×302) spk fwd rev turn eat age aro fear les?}` + shared `WORM` connectome.

---

## §3 — Event mechanisms

- `logEvent` (480): structured `{id,d,type,text,x,y,ids{p|sett|fac},actor,victim,why{numbers},
  causeTxt|causeId}` + reverse `cons[]` on the parent **capped at 8** (493). Types: found, war,
  battle, politics, faith, calamity, tech, life, misc.
- **Caps**: events 5,200 (best 20,000) with 800 oldest life/misc dropped then tail-keep (499–508);
  save-time slice to last 3,000 (best 12,000) (8173). UI copes with dangling ids ("crumbled to
  dust", 7803).
- Causal anchors held in state: `w.ev` (war), `st.plagueEv`, `st.famEv`, `m.causeEvId`,
  `sim.beast.evId`, `sim.fireEvId` (singleton — every town fire cites the *latest* wildfire, 3398).
- Log **throttles** discard causal data by design: famine ≥2y apart (3480), town fire ≥120d
  (3397), beast kills every 4th (1750), plot whispers 3y (2041), no-march 1/y (5185), lake 8y
  (5584), starvation/gate-shame 1/y (4201/4724).
- Parallel text records duplicating history: `bio[]` per person (cap 44/160), settlement `log[]`,
  relic `log[]`, books (cap 40), eras, `sim.stats` series (cap 1,200y, zero-backfilled at 2527).
- Personal memories (`remember` 2869): cap 8/16, ×0.97 yearly decay, prune <0.05; inherited as
  distorted told-tales at 60% (2890); these drive grievance (2900) — memory IS causal state here.

## §4 — Relationship kinds

Marriage `spouse`; parentage `fa/mo/kids`; typed bonds `frs[]` friend/comrade/cred(+amt,bad)
(rival named in comment, never constructed); master/apprentice `_app/_stu`; teacher via `teach`;
steward `st.steward`+`loyal`; leadership `f.leader`, dynasty `f.dyn`; faction membership `p.fac`;
residence `p.home`, tenancy `hst/hx/hy`; army `inArmy`+`members[]`+`gen`; crew `sea`+`members[]`;
tunnel crew `tun`+`workers[]`; diplomacy `sim.relations` scalar + `f.rel{}` + shared `wars{}`;
grudge `p.grudge`(faction); memory blame `m.fac/m.p`; livestock `a.dom`(settlement, not person);
animal heredity `genes/gen`; predator↔prey (transient, not persisted); territory `settleId/
terrId`; trade partners `st.trade.partners`; knowledge-of `st.known/f.known`; religion
`st.religion/f.relig`+`founder`; currency `issuer`; relic `holder/sett`+provenance; graves
`gy.graves[].p`; conspiracy `plot.fol[]`; event causality `causeId/cons`.

## §5 — Persistence

JSON v2 (`saveString` 8133): world arrays as base64, **elevation quantized to Uint8**, cultures,
veg via `vegSer`, sim payload with **souls pruned to 4,000 non-notables** (best ∞, 8139), animals
**stripped of brains** (8151, regenerated lazily by the *renderer* at 8333), settlements stripped
of `_*`/inv/price/trade (econ re-packed via `econSerSt`), factions stripped of `_*` and war `ev`
refs, **events sliced to 3,000/12,000** (8173). Load (8177): **reseeds the global RNG** `R=
mulberry32(hashStr(seedStr)^0x9e37)` (8180) — post-load runs diverge from unbroken runs by
construction. localStorage manual save + 3-minute autosave. Caches all rebuilt lazily. The save
is an approximation, not a resumption.

---

## §6 — Cheats, approximations, schedules, prunes → emergent replacements

Each entry: **site → replacement paragraph.** (Directive-named items first.)

**6.1 Mode flags** — `bootWorld(seed,W,H,histYears,brains,pro,best)` (8249–8262), UI wiring
8101–8130, branches at 499, 529, 2125, 2875, 2925, 3464, 4001, 4251, 5220, 5375, 5481, 5700,
7985–7986, 8139, 8173, 8206–8207.
→ Deleted entirely. One mode = the union of the best behaviors: pro's causal mechanics
(grievance wars, plots, stress, grief) run always; best's "prune nothing" is the storage design
(EVENT_MODEL.md); brains-level decision fidelity for every agent (one brain architecture,
species-parameterized — whether a literal connectome is kept is a Stage 3 design choice, but
fidelity never branches). `histYears` pre-run is deleted: old worlds are produced by running the
engine longer, headless.

**6.2 Souls pruning** (4133: >22,000 → delete 2,000 non-notables; save cap 4,000 at 8139; soul
record itself lossy, bio kept only for notables at 4131).
→ Historical-entity registry: death moves a person's full state (memories, relationships, skills,
possessions) to cold storage under the same permanent ID; nothing is summarized or dropped;
"notable" becomes a computed display rank, not a retention class.

**6.3 Event caps** (499–508, 8173; cons cap-8 at 493; log throttles §3).
→ Append-only segmented event store tiered to disk; consequences as a reverse index (unbounded);
throttling becomes a *display* concern — every famine day, gate shame, and beast kill is recorded;
the chronicle UI ranks and folds, storage never does.

**6.4 Vegetation density fields** (`veg/vsp/litter/soilN/seedB/vgen/blight` 1007–1016; 21-day
stripes 1074; 420-sample dispersal 1196; one species per cell; one gene scalar per cell).
→ Individual plants with species/age/growth/health/roots/seeds, competing for the cell's real
water/light/nutrients; a cell hosts many plants; genes ride individuals; dispersal follows from
each plant's actual seed production, wind, water and the animals that really ate there; forest
cover, biome labels, and the seed bank become `derive(plants)`. The prototype's suitability
curves, litter→soil cycle, fire response, and succession *shapes* are ported as the plant model's
parameters.

**6.5 `sim.beast` singleton + `a.legend`** (335/977 cap:1 species; 1712–1777 spawn/update/fight;
1727/1761 self-repair; 1729 mercy rule; 1752 teleport; 2108–2117 promotion thresholds + hp buff;
1926 slot reset "room for the next terror").
→ No singleton, no flag, no cap of one, no teleporting, no stat buffs. A predator that kills
people is just a predator whose kill events accumulate; "legend" is a derived reputation computed
from kill events witnessed and retold (memories with distortion); the town's response (watch,
cull, bait, epithets, naming) reads that reputation. Any number of man-eaters can earn names
simultaneously; slaying one is a real fight resolved by the combat model.

**6.6 Prophet by threshold** (5700–5715: `!pro && chance(min(0.12,pop/6000))`, max-piety scan,
`pie>0.8`, hardcoded color; grief-prophets pro-only at 2125; spread rolls 5723 d<30 & chance(0.15),
caravan 30%, ship 30%, faction adoption from capital 5727).
→ Individuals hold beliefs as data; experiences (grief, plague, famine, wonder) generate and
strengthen beliefs; people transmit beliefs through real speech contacts (co-location,
households, teaching, trade); "a religion" is a derived cluster over the belief graph, its
founder the person who actually originated the tradition, its spread the sum of real conversions;
color/name derive from culture. Grief-driven origination (the pro path) generalizes to the only
path.

**6.7 `pick()`-based social facts** (proQuarterlySocial 1964–1978; random victims: beast 1747,
madness 2157, fire 3396, famine 3485, plague 3491, tunnel collapse 3757–3758; random gate site
3804; random marriage pairing 5520 + founder marriages 896 + random parentage 899–901; crew/
apprentice/patron/plotter by Map-iteration order 1643, 2177, 2345, 2843, 3709).
→ Relationships change only through encounters that occurred: co-work on the same trip/site,
shared household, shared institution attendance, army comradeship — each encounter is an event
and bonds strengthen from them. Victims are determined by who was really there (spatial truth):
the fire kills whoever was in the building; plague infects along real contact chains (pathogen
instances); the collapse kills the crew member working the collapsing section. Marriages come
from courtship between people who actually met. All selections iterate in deterministic ID order.

**6.8 Renderer mutating state** (lesion tool 7731–7748; lazy brain creation in `drawWormRaster`
8333; panel-open lazy econ/money init with panel-time timestamps 2588/2607; `smooth` writing
rx/ry onto entities 5935, serialized at save; selected-animal think-boost 4941–4942; UI-owned
tick loop dropping backlog 8308; hardware-dependent speed caps 7982).
→ Inspection API is read-only by type; render interpolation state lives in the renderer keyed by
entity ID; all lazy init moves to entity construction; debug interventions (lesioning) become an
explicit engine command channel, recorded as events, never a render side-effect; the sim clock is
engine-owned; per-entity evaluation cadence never reads selection/observation (Test H).

**6.9 Probability-gated history** (full verified list): peace `chance(0.15)` 5214; rout
`chance(0.25)` 5035; raze coin-flip 5065; conquest flat 12%/60%/70% rolls 5105–5129; discovery
gate `discRipe` 2728; wildfire ignition flat p 3316; building-fire death 25% 3396; famine/plague
victim rolls 3484/3491; per-person daily mortality roll 3515; plague ashore 35% 1588 / caravan
35% 5241 / outbreak `chance(0.02*crowd)` + one-per-year `break` 5774–5776; religion rolls 1586/
5239/5723; drought global `chance(0.07)` 5767; ship sinking 1547; blight 1229; beast kill/slay
rolls 1746/1763; stress-kill coin flip 2158; migration `chance(0.35)` 5341; births `rnd()<0.024`
5350; taming `chance(0.5)` 5382; marriage `chance(0.4)` 5520; raids `chance(0.22)` 5675;
re-muster `chance(0.7)` 5693; species re-immigration `chance(0.3)` at map edge 5746; work-trip
`chance(0.12)` 4002; hunter dice-chains 4047–4069; kill-at-adjacency `chance(0.35)` 4690; herd
defense 4695/4698; watch saves 4714; fence leap 4746; man-eating chain 4764–4815; bait spring
4860; want-death rolls 4882; croc rolls 4531–4540; rivalry 4676–4681; lifespan re-rolled daily
`age>ri(3000,4600)` 4192; reproduction pulse 4263–4284; scar/lake/canal healing rolls 5558–5647;
gate-closing roll 5322; sanitation 5336; commodity-money and trade-route RNG tie-breaks 2510/2327.
→ Rule (SIMULATION_MODEL.md "causality bar"): put the missing state into the world — morale/
formation/terrain for battles and peace (wars end when a side's people/leadership actually can't
or won't continue); pathogen instances with contact transmission; per-animal aging and condition;
courtship/pregnancy on real co-location; hydrology for drought (regional water balance, not a
coin); per-cell fuel/wind/humidity for ignition and spread with lightning strikes as located
events; discovery from practice-hours + materials + prior knowledge of a real person. Residual
randomness only perturbs outcomes of situations that exist.

**6.10 Aggregate substitutes** (economy from profession head-counts 2234–2251, 3459–3460; siege
defenders formula 5052; conquest flat fractions 5107–5111; tech combat multipliers 5029; livestock
meat inflow 3405–3421; fishing boat flat yield 1554; wealth/treasury scalar purses; debasement
conjuring +300×lost 2575; species `cap` constants 4271/925; `_gameMod/_forestMod` snapshots 5499;
woodcutter shortcut >1800 pop 3464; crop health collapsed to `_cropH` 1250; contact/drift scalars
3197; `knownFacPop` fuzz per call 3095).
→ Goods are lots with provenance produced by real work acts of real people at real sites (a trip
that happened yields what it gathered); garrisons are the people present; combat effects flow
from actual equipment items; herds are the animals owned; carrying capacity is derived from
habitat, not authored caps; scarcity is what the spatial index actually finds; heard-strength is
a stable belief updated by real reports, not per-call fuzz; money is conserved through a real
ledger (minting consumes metal).

**6.11 Scheduling as world behavior** (fortnightly reproduction pulse 4263; monthly/quarterly/
yearly modulo offsets 3295–3298; 90-day ghost-berth sweeps 3518–3522; exact-age transitions 3526;
40-year assimilation 3203; 720-day apprenticeships 2825; era 10-year blocks 1855; one-discovery-
per-town-month 2751; one-culture-fork-per-year 3245; one-plague-per-year 5776; one-relic-per-year
cap 9 2169–2182; fire spread halt >350 tiles 3336; pathBudget 6/tick 3278; teleport-home 3997;
2-tiles-per-step movement).
→ Scheduling remains a computation tool but never synchronizes the *world*: animals breed when
individuals mate; discoveries happen when a person's conditions are met (no monthly slot, no
world quota); stuck agents path or struggle, never teleport; fire spreads by physics regardless
of how much is burning (cost handled by chunked cadence, not a halt); references to dead ships/
projects clear on the death event, not a 90-day sweep.

**6.12 Environment singletons** (one global `weather{type,t}` + `drought` bool 979/5767; open-map
species immigration 5746; road/bridge as usage counters 307/469; lakes recede/refill by rolls
5583–5596).
→ Weather is regional fields (climate/), drought is derived regional water balance, roads and
bridges are built/eroding objects (usage still drives *creation decisions* by people), lakes rise
and fall because water volumes do (water/), immigration only across real map edges from modeled
outside populations or not at all (closed world documented).

---

## §7 — Determinism findings

Single global mulberry32 stream `R` (215) for gen + sim; renderer never draws from it (all 7
`Math.random` sites are visual or pre-seed selection, verified 6107–6127, 8103–8450). But:
per-load reseed (8180) breaks save/continue equivalence; `wormStep` draws ~2 values/neuron/step
making replay hyper-sensitive to animal order; several selections depend on `sim.people` Map
insertion order (2345, 2843, 3709, 1643, 2001, 2177); UI-driven tick loop drops backlogged days
under load (8308) and hidden tabs tick on a different budget (8433); panel-open lazy inits
timestamp state (2588/2607); selected-animal think cadence (4941). The engine replaces all of
this with the per-entity stream scheme in DETERMINISM.md.

## §8 — Bugs found (report-only; do not port)

1. 1155: blight-clear roll computes `chance(0.06*dt/VEG_STRIDE*VEG_STRIDE)` = `chance(1.26)` —
   always true; blight self-clears every update and its spread branch is mostly unreachable.
2. 1875: `makeBook` event filter ends `||areAtWar!==undefined` (always true) — faction filter is
   a no-op; books sample all factions' events.
3. Save round-trip: elevation quantized to 8-bit, brains stripped and regenerated by the renderer,
   RNG reseeded — loads are approximations by design (documented, but effectively a bug against
   the directive's standards).

## §9 — Rule conflicts found during audit

1. **Rust vs C++** — the directive contradicts itself (§0 vs appended text). Escalated:
   DECISIONS D-001.
2. **"Every plant is an entity" at prototype scale** — a 300×190 world fully treed implies ~10⁵–10⁶
   plants; directive Test A explicitly demands 10k-tree worlds and §2.12 forbids density
   substitutes. No conflict, but flagged: Stage 3 must budget for millions of plant entities
   (ENTITY_MODEL.md sets the compact layout).
3. **Worm brains vs one-architecture rule** — §4.5 demands one decision architecture parameterized
   per species; the prototype has four minds (instinct/salmon/croc/worm) plus a mode flag. The
   engine unifies: one perception→memory→utility pipeline whose parameters (and optionally a
   neural policy) vary by species. No fidelity switch survives.
4. **Open vs closed world** — prototype re-immigrates extinct species from the map edge (5746).
   The directive is silent. Decision deferred to Stage 3 (either model an outside or accept
   extinction as permanent); recorded as an open question.
