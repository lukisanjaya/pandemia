ALTER TABLE village_data ADD COLUMN city_id BIGINT NOT NULL REFERENCES cities(id);
ALTER TABLE village_data ADD COLUMN meta TEXT[] NOT NULL DEFAULT '{}';