-- Wiki-backed zone enrichment for the original nickname-keyed timer catalog.
-- Preserve user/imported edits and any zone already supplied by the user.
UPDATE spawn_timer_overrides
SET zone_name='Temple of Veeshan', source='p99-wiki', updated_at=CURRENT_TIMESTAMP
WHERE zone_name IS NULL AND source IN ('bundled-json','p99-wiki')
  AND nickname IN ('eashen','telk','cekenr','ltk','gozz','vyemm','zlexak','dagarn','jorrleag',
                   'ladym','ladyn','feshlak','sevalak','koi','ikatiar','vulak','kreizenn','aary','dozekar');

UPDATE spawn_timer_overrides
SET zone_name='Plane of Fear', source='p99-wiki', updated_at=CURRENT_TIMESTAMP
WHERE zone_name IS NULL AND source IN ('bundled-json','p99-wiki')
  AND nickname IN ('draco','fright','dread','terror');

UPDATE spawn_timer_overrides
SET zone_name='Plane of Mischief', source='p99-wiki', updated_at=CURRENT_TIMESTAMP
WHERE zone_name IS NULL AND source IN ('bundled-json','p99-wiki')
  AND nickname IN ('grink','brenn','glonk','22','grenn');

UPDATE spawn_timer_overrides
SET zone_name='Kael Drakkel', source='p99-wiki', updated_at=CURRENT_TIMESTAMP
WHERE zone_name IS NULL AND source IN ('bundled-json','p99-wiki')
  AND nickname IN ('statue','vindi','kdt');

UPDATE spawn_timer_overrides
SET zone_name='Dragon Necropolis', source='p99-wiki', updated_at=CURRENT_TIMESTAMP
WHERE zone_name IS NULL AND source IN ('bundled-json','p99-wiki')
  AND nickname IN ('vilefang','zlandicar');

UPDATE spawn_timer_overrides
SET zone_name='Plane of Hate', source='p99-wiki', updated_at=CURRENT_TIMESTAMP
WHERE zone_name IS NULL AND source IN ('bundled-json','p99-wiki')
  AND nickname IN ('inny','magi');

UPDATE spawn_timer_overrides SET zone_name='Western Wastes', source='p99-wiki', updated_at=CURRENT_TIMESTAMP WHERE nickname='kozz' AND zone_name IS NULL AND source IN ('bundled-json','p99-wiki');
UPDATE spawn_timer_overrides SET zone_name='The Wakening Land', source='p99-wiki', updated_at=CURRENT_TIMESTAMP WHERE nickname='wuoshi' AND zone_name IS NULL AND source IN ('bundled-json','p99-wiki');
UPDATE spawn_timer_overrides SET zone_name='Kedge Keep', source='p99-wiki', updated_at=CURRENT_TIMESTAMP WHERE nickname='phinny' AND zone_name IS NULL AND source IN ('bundled-json','p99-wiki');
UPDATE spawn_timer_overrides SET zone_name='Karnor''s Castle', source='p99-wiki', updated_at=CURRENT_TIMESTAMP WHERE nickname='vs' AND zone_name IS NULL AND source IN ('bundled-json','p99-wiki');
UPDATE spawn_timer_overrides SET zone_name='Old Sebilis', source='p99-wiki', updated_at=CURRENT_TIMESTAMP WHERE nickname='trakanon' AND zone_name IS NULL AND source IN ('bundled-json','p99-wiki');
UPDATE spawn_timer_overrides SET zone_name='Skyfire Mountains', source='p99-wiki', updated_at=CURRENT_TIMESTAMP WHERE nickname='talendor' AND zone_name IS NULL AND source IN ('bundled-json','p99-wiki');
UPDATE spawn_timer_overrides SET zone_name='Dreadlands', source='p99-wiki', updated_at=CURRENT_TIMESTAMP WHERE nickname='gore' AND zone_name IS NULL AND source IN ('bundled-json','p99-wiki');
UPDATE spawn_timer_overrides SET zone_name='Timorous Deep', source='p99-wiki', updated_at=CURRENT_TIMESTAMP WHERE nickname='faydedar' AND zone_name IS NULL AND source IN ('bundled-json','p99-wiki');
UPDATE spawn_timer_overrides SET zone_name='Iceclad Ocean', source='p99-wiki', updated_at=CURRENT_TIMESTAMP WHERE nickname IN ('lodi','stormfeather') AND zone_name IS NULL AND source IN ('bundled-json','p99-wiki');
UPDATE spawn_timer_overrides SET zone_name='Velketor''s Labyrinth', source='p99-wiki', updated_at=CURRENT_TIMESTAMP WHERE nickname='velk' AND zone_name IS NULL AND source IN ('bundled-json','p99-wiki');
UPDATE spawn_timer_overrides SET zone_name='Emerald Jungle', source='p99-wiki', updated_at=CURRENT_TIMESTAMP WHERE nickname='sev' AND zone_name IS NULL AND source IN ('bundled-json','p99-wiki');
UPDATE spawn_timer_overrides SET zone_name='Temple of Droga', source='p99-wiki', updated_at=CURRENT_TIMESTAMP WHERE nickname='angry' AND zone_name IS NULL AND source IN ('bundled-json','p99-wiki');
UPDATE spawn_timer_overrides SET zone_name='Great Divide', source='p99-wiki', updated_at=CURRENT_TIMESTAMP WHERE nickname='narandi' AND zone_name IS NULL AND source IN ('bundled-json','p99-wiki');
UPDATE spawn_timer_overrides SET zone_name='Western Wastes', source='p99-wiki', updated_at=CURRENT_TIMESTAMP WHERE nickname='scout' AND zone_name IS NULL AND source IN ('bundled-json','p99-wiki');
UPDATE spawn_timer_overrides SET zone_name='Cobalt Scar', source='p99-wiki', updated_at=CURRENT_TIMESTAMP WHERE nickname='kd' AND zone_name IS NULL AND source IN ('bundled-json','p99-wiki');

INSERT OR IGNORE INTO schema_migrations(version,name)
VALUES(46,'spawn timer wiki zones');
