export interface BootstrapStatus { appVersion:string; platform:string; databasePath:string; databaseReady:boolean; schemaVersion:number; legacyDatabase:boolean }
export interface ModelPackStatus { installed:boolean; valid:boolean; name?:string; version?:string; path?:string; sourceUrl?:string; modelCount:number; fileCount:number; bytes:number; verifiedAt?:string; error?:string }
export interface Member { id:number; name:string; active:boolean }
export interface Loot { id:number; happenedAt:string; itemName:string; itemId?:number; mobName?:string; looterName?:string; valuePp?:number; valueBasis?:string; valueSamples?:number; splitListed:boolean; attendees:string[] }
export interface Split { key:string; itemName:string; addedAt:string; mobName?:string; looterName?:string; payoutValuePp?:number; marketValuePp?:number; marketValueBasis?:string; marketValueSamples?:number; attendees:string[] }
export interface SplitSaleReconciliationSale { key:string; valuePp:number; note:string }
export interface SplitSaleReconciliationRequest { sales:SplitSaleReconciliationSale[] }
export interface TrackedLoot { id:number; sourceLootId?:number; happenedAt:string; trackedAt:string; itemName:string; mobName?:string; looterName?:string; valuePp?:number; valueBasis?:string; valueSamples?:number; attendees:string[] }
export interface LinkedLoot { id:number; happenedAt:string; channel:"group"|"guild"; speakerName:string; itemName:string; itemId?:number; valuePp?:number; valueBasis?:string; count30d:number }
export interface ActivityLoot { id:number; happenedAt:string; character:string; itemName:string; itemId?:number; looterName:string; sourceFile:string; valuePp?:number; valueBasis?:string; valueSamples:number }
export interface ActivityMob { id:number; happenedAt:string; character:string; mobName:string; killerName?:string; sourceFile:string }
export interface ActivityOffer { id:number; happenedAt:string; character:string; offererName:string; itemName:string; itemId?:number; sourceFile:string; valuePp?:number; valueBasis?:string; valueSamples:number }
export interface ActivityLevel { id:number; happenedAt:string; character:string; level:number; direction:"gained"|"lost"; sourceFile:string }
export interface ActivityHistorySnapshot { loot:ActivityLoot[]; mobs:ActivityMob[]; offers:ActivityOffer[]; levels:ActivityLevel[] }
export interface DeathReport { id:number; happenedAt:string; character:string; killerName:string; sourceFile:string; contextCount:number }
export interface DeathReportEntry { sequenceNumber:number; rawLine:string }
export interface DeathReportDetail extends DeathReport { rawLine:string; sourceOffset:number; entries:DeathReportEntry[] }
export interface ClericHealCall { id:number; happenedAt:string; character:string; clericName:string; callNumber:number; targetName?:string; channel:"group"|"guild"; message:string; sourceFile:string }
export interface GuildSlowCall { id:number; happenedAt:string; character:string; speakerName:string; mobName:string; message:string; sourceFile:string }
export interface ChTrainingLine { lineNumber:number; happenedAt?:string; rawLine:string; status:"recognized"|"other"|"ignored"; parserEvent:string; summary:string }
export interface ChTrainingReport { activeCharacter:string; lineCount:number; recognizedCount:number; ignoredCount:number; calls:ClericHealCall[]; lines:ChTrainingLine[] }
export interface ClericHealReplayCall extends ClericHealCall { gapSeconds?:number; clericGapSeconds?:number }
export interface ClericHealReplaySummary { startedAt:string; endedAt:string; durationSeconds:number; callCount:number; healerCount:number; averageGapSeconds:number; longestGapSeconds:number }
export interface ClericHealReplayFile { formatVersion:number; title:string; savedAt:string; character:string; targetMob:string; summary:ClericHealReplaySummary; calls:ClericHealReplayCall[]; path?:string }
export interface ClericHealReplayEntry extends ClericHealReplaySummary { fileName:string; path:string; title:string; savedAt:string; character:string; targetMob:string }
export interface SystemTaskStatus { id:string; label:string; state:"running"|"completed"|"failed"; detail:string; completed?:number; total?:number; startedAt:string; finishedAt?:string }
export interface GlobalStatusSnapshot { tasks:SystemTaskStatus[]; theme?:string; activeCharacter?:string; preferredTargetCharacter?:string; preferredTargetEncounterId?:number; currentWeaponLoadout?:CurrentWeaponLoadout; damageEncounters:DamageEncounter[] }
export interface DamageParticipant { name:string; totalDamage:number; hitCount:number; firstDamageAt:string; lastDamageAt:string }
export interface DamageTarget extends DamageParticipant { maxHit:number }
export interface DamageSpellMetric { playerName:string; spellName:string; procCount:number; directProcDamage:number; dotDamage:number; dotTickCount:number; procDotDamage:number; totalProcDamage:number }
export interface ProcEvidenceMessage { sourceOffset:number; role:"preceding-context"|"landing"|"confirmation"|"landing-confirmation"|"supporting"; rawLine:string }
export interface ProcEvidenceOccurrence { id:number; spellName:string; targetName:string; casterName:string; happenedAt:string; directDamage:number; sourceName?:string; sourceFile:string; landingSourceOffset:number; attributionSourceOffset:number; evidenceSource:"source-log"|"retained-events"|"unavailable"; messages:ProcEvidenceMessage[] }
export interface ProcEvidenceReport { encounterId:number; playerName:string; occurrences:ProcEvidenceOccurrence[] }
export interface TrackedSpellActivity { id:number; spellName:string; targetName:string; casterName:string; sourceKind:"direct"|"proc"|"item_click"|"unknown"; sourceName?:string; happenedAt:string }
export interface ActiveDot { id:number; spellName:string; targetName:string; casterName:string; attributionMethod:"item_glow"|"direct_cast"|"proc"|"next_attack"|"unknown"; landedAt:string; expiresAt:string; damagePerTick:number; tickIntervalSeconds:number; totalTicks:number; ticksApplied:number; inferenceEnabled:boolean; status:string }
export interface DamageEncounter { id:number; character:string; mobName:string; startedAt:string; endedAt?:string; lastDamageAt:string; totalDamage:number; meleeDamage:number; spellDamage:number; hitCount:number; maxHit:number; incomingDamage?:number; incomingHitCount?:number; incomingMaxHit?:number; procCount?:number; directProcDamage?:number; dotDamage?:number; procDotDamage?:number; totalProcDamage?:number; spellMetrics?:DamageSpellMetric[]; trackedSpells?:TrackedSpellActivity[]; outcome:"active"|"slain"|"playerDeath"|"disengaged"; sourceFile:string; weapons:string[]; players:DamageParticipant[]; damageTargets?:DamageTarget[]; activeDots?:ActiveDot[]; isProtected?:boolean; protectedAt?:string }
export interface DamageEvent { id:number; happenedAt:string; attacker:string; damageType:"melee"|"spell"; source?: "explicit"|"dot"|"proc"; attack:string; damage:number; primaryWeapon?:string; primaryItemId?:number; secondaryWeapon?:string; secondaryItemId?:number }
export interface DamageAttackTypeMetric { attack:string; damageType:"melee"|"spell"; totalDamage:number; hitCount:number; maxHit:number }
export interface IncomingDamageEvent { id:number; happenedAt:string; attacker:string; target:string; attack:string; damage:number }
export interface DamageEncounterDetail extends DamageEncounter { events:DamageEvent[]; incomingEvents:IncomingDamageEvent[] }
export interface CurrentWeaponLoadout { character:string; capturedAt:string; primary?:string; primaryItemId?:number; secondary?:string; secondaryItemId?:number }
export interface SplitPayout { name:string; paidAt:string }
export interface History { id:number; itemName:string; mobName?:string; looterName?:string; valuePp:number; disposition:string; payoutStatus:"pending"|"completed"; note:string; completedAt:string; paidAt?:string; attendees:string[]; payouts:SplitPayout[] }
export interface MasterItem { id:number; name:string; iconId?:number; valuePp:number; valueBasis?:string; count30d:number; lastSeen:string; manual:boolean; source:string }
export interface InventoryItem { character:string; importedAt:string; id:number; location:string; itemName:string; itemId?:number; iconId?:number; material?:number; idFile?:string; color?:number; itemType?:number; count:number; slots?:number; valuePp?:number; valueBasis?:string; valueSamples?:number }
export interface WardrobeCatalogItem { id:number; peqId?:number; name:string; iconId?:number; itemType?:number; slots:number; classes:number; races:number; weight:number; ac:number; hp:number; mana:number; strength:number; stamina:number; agility:number; dexterity:number; wisdom:number; intelligence:number; charisma:number; magicResist:number; fireResist:number; coldResist:number; diseaseResist:number; poisonResist:number; attack:number; haste:number; manaRegen:number; damageShield:number; damage:number; delay:number; clickName:string; procName:string; wornName:string; focusName:string; material?:number; idFile?:string; color?:number; setNames:string[] }
export interface WardrobeSetSummary { name:string; source:string; classes:string[]; itemCount:number }
export interface WardrobeSetItem { slot:string; item:WardrobeCatalogItem }
export interface QuestCatalogItem { entryId:number; category:"plane_of_sky"|"velious_armor"|"epic"; className:string; questName:string; rewardItemId?:number; rewardName:string; rewardIconId?:number; componentId:number; itemId?:number; itemName:string; iconId?:number; quantity:number; slot:string; faction:string; note:string; sourceUrl:string; valuePp?:number; valueBasis?:string; valueSamples:number }
export interface Spell { character:string; importedAt:string; slot?:number; spellName:string }
export interface CharacterProfile { character:string; race:string; gender:"m"|"f"; classCode:string; level?:number; parsedLevel?:number; levelOverride?:number; levelSource:"logs"|"manual"|"unknown"; updatedAt?:string; levelObservedAt?:string }
export interface SpellClassInfo { name:string; level:number }
export interface SpellEffectInfo { slot:number; description:string }
export interface SpellInfo { spellName:string; wikiUrl:string; description:string; classes:SpellClassInfo[]; effects:SpellEffectInfo[]; mana:string; skill:string; castingTime:string; recastTime:string; fizzleTime:string; resist:string; range:string; targetType:string; spellType:string; duration:string; reagent:string; focus:string; whereToObtain:string; castOnYou:string; castOnOther:string; wearsOff:string; damageKind:"dot"|"direct"|"hybrid"|"non_damage"|"unknown"; directDamage?:number; damagePerTick?:number; tickCount?:number; tickIntervalSeconds:number; totalDotDamage?:number; fetchedAt:string; stale:boolean }
export interface SpellCatalogStatus { cachedCount:number; processed:number; saved:number; failed:number; refreshing:boolean; startedAt?:string; lastRefreshAt?:string; lastError?:string }
export interface WtsGroup { id:number; character:string; name:string; createdAt:string; updatedAt:string; items:string[]; itemIds:(number|null)[] }
export interface Alias { alias:string; canonical:string }
export interface AppLog { id:number; happenedAt:string; level:string; area:string; message:string }
export interface ImportRecord { id:number; happenedAt:string; fileName:string; status:string; reviewUrl?:string; detail?:string }
export interface MerchantListingItem { id:number; itemName:string; itemId?:number; askingPricePp?:number; marketValuePp?:number; marketValueBasis?:string; marketCount30d:number }
export interface MerchantMessage { id:number; happenedAt:string; kind:"wts"|"wtb"|"tell"; speakerName:string; message:string; items:MerchantListingItem[] }
export type CompoundSource="personal"|"split"|"shared";
export interface CompoundComponent { id:string; itemId:number|null; itemName:string; required:number; received:number; valuePp:number; source:CompoundSource; sourceRef:string|null; contributors:string[]; note:string }
export interface CompoundTemplateComponent { itemId:number|null; itemName:string; required:number; valuePp:number }
export interface CompoundTemplate { id:string; name:string; itemId:number|null; builtIn?:boolean; components:CompoundTemplateComponent[] }
export interface CompoundProject { id:string; itemId:number|null; name:string; note:string; status:"building"|"ready"|"hold"|"sold"; templates:string[]; components:CompoundComponent[]; soldAt?:string; saleValuePp?:number; saleNote?:string }
export interface CompoundWorkspace { projects:CompoundProject[]; templates:CompoundTemplate[]; activeId:string|null }
export type Runner=(action:string,payload?:Record<string,unknown>)=>Promise<unknown>;
export interface AppSnapshot { settings:Record<string,string>; members:Member[]; loot:Loot[]; splits:Split[]; tracked:TrackedLoot[]; linkedLoot:LinkedLoot[]; history:History[]; items:MasterItem[]; inventory:InventoryItem[]; questCatalog:QuestCatalogItem[]; spells:Spell[]; characterProfiles:CharacterProfile[]; wts:WtsGroup[]; aliases:Alias[]; mobs:string[]; logs:AppLog[]; imports:ImportRecord[]; merchant:MerchantMessage[]; deathReports:DeathReport[]; damageEncounters:DamageEncounter[]; damageEncounterCount:number; damageDistinctMobCount:number; damageAttackTypes:DamageAttackTypeMetric[]; clericHealCalls?:ClericHealCall[]; guildSlowCalls?:GuildSlowCall[]; currentWeaponLoadout?:CurrentWeaponLoadout; compound:CompoundWorkspace }
export interface DatabaseStorageCategory { key:string; label:string; description:string; rowCount:number; estimatedPayloadBytes:number; oldestAt?:string; newestAt?:string; retentionSupported:boolean; summaryPreserved:boolean }
export interface DatabaseStats { databasePath:string; databaseBytes:number; walBytes:number; pageSize:number; pageCount:number; freelistPages:number; reclaimableBytes:number; schemaVersion:number; journalMode:string; integrity:string; generatedAt:string; categories:DatabaseStorageCategory[] }
export interface DatabaseCleanupPreview { cutoff:string; outgoingRows:number; incomingRows:number; totalRows:number; estimatedPayloadBytes:number; oldestAt?:string; newestAt?:string; encounterSummariesPreserved:boolean }export interface ReplayFightSummary { mobName:string; startedAt:string; durationSeconds:number; totalDamage:number; participantCount:number; eventCount:number; procCount:number; dotDamage:number }
export interface FightPurgePreview { cutoff:string; eligibleEncounters:number; protectedEncounters:number; outgoingRows:number; incomingRows:number }
export interface ProtectedFight { id:number; character:string; mobName:string; startedAt:string; endedAt?:string; totalDamage:number; hitCount:number; outcome:string; protectedAt:string }
export interface FightPurgeResult { deletedEncounters:number; cutoff?:string; backupPath?:string }
export interface ReplaySource { encounterId:number; sourceFile:string; firstSourceOffset:number; lastSourceOffset:number; captureMode:"source-window"|"retained-events"|string }
export interface ReplayFile { formatVersion:number; title:string; savedAt:string; activeCharacter:string; projectAllTicks:boolean; summary:ReplayFightSummary; source:ReplaySource; logLines:string[]; notes:string; coachReviews:ProcCoachSavedReview[] }
export interface ReplayFileEntry extends ReplayFightSummary { fileName:string; path:string; title:string; savedAt:string; activeCharacter:string; captureMode:string; lineCount:number; coachReviewCount:number }
export type ProcCoachSourceKind="proc"|"direct_cast"|"item_click"|"unattributed_spell"|"not_a_proc";
export interface ProcCoachStatus { configured:boolean; model:string; secureStore:string }
export interface ProcCoachFinding { id:string; lineNumbers:number[]; targetName:string|null; probableCaster:string|null; spellName:string|null; sourceKind:ProcCoachSourceKind; parserVerdict:"agrees"|"disagrees"|"uncertain"; confidence:"high"|"medium"|"low"; suggestedDamage:number|null; explanation:string; evidence:string[]; proposedCorrection:string|null }
export interface ProcCoachReview { model:string; reviewedAt:string; summary:string; findings:ProcCoachFinding[] }
export interface ProcCoachFindingDecision { findingId:string; verdict:"accepted"|"rejected"; sourceKind:ProcCoachSourceKind; casterName:string|null; spellName:string|null; note:string }
export interface ProcCoachSavedReview { reviewedAt:string; model:string; agentSummary:string; findings:ProcCoachFinding[]; decisions:ProcCoachFindingDecision[] }
export interface DotTrainingLine { lineNumber:number; sourceOffset:number; happenedAt?:string; rawLine:string; status:"proc"|"dot"|"recognized"|"ignored"|"invalid"; parserEvent:string; summary:string; decisions:string[] }
export interface DotTrainingApplication { id:number; encounterId:number; spellName:string; targetName:string; casterName:string; attributionMethod:"item_glow"|"direct_cast"|"proc"|"next_attack"|"unknown"; landedAt:string; expiresAt:string; damagePerTick:number; tickIntervalSeconds:number; totalTicks:number; ticksApplied:number; inferenceEnabled:boolean; status:string; sourceOffset:number }
export interface DotTrainingProc { id:number; encounterId:number; spellName:string; targetName:string; casterName:string; happenedAt:string; directDamage:number; landingSourceOffset:number; attributionSourceOffset:number }
export interface DotTrainingEvent { id:number; encounterId:number; happenedAt:string; attacker:string; damageType:string; attack:string; damage:number; inferred:boolean; sourceKind:"explicit"|"dot"|"proc"; tickIndex?:number; sourceOffset:number }
export interface DotTrainingIncomingEvent { id:number; encounterId:number; happenedAt:string; attacker:string; target:string; attack:string; damage:number; sourceOffset:number }
export interface DotTrainingParticipant { name:string; totalDamage:number; hitCount:number }
export interface DotTrainingEncounter { id:number; mobName:string; startedAt:string; endedAt?:string; lastDamageAt:string; totalDamage:number; meleeDamage:number; spellDamage:number; hitCount:number; outcome:string; participants:DotTrainingParticipant[]; events:DotTrainingEvent[]; incomingEvents:DotTrainingIncomingEvent[]; dots:DotTrainingApplication[]; procs:DotTrainingProc[] }
export interface DotTrainingReport { activeCharacter:string; lineCount:number; recognizedCount:number; ignoredCount:number; dotProfileCount:number; combatProfileCount:number; projectedAllTicks:boolean; projectedThrough?:string; lines:DotTrainingLine[]; encounters:DotTrainingEncounter[]; warnings:string[] }
export type LoadingState<T>={kind:"loading"}|{kind:"ready";value:T}|{kind:"error";message:string};
