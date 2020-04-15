CREATE TABLE cities (
  id BIGSERIAL PRIMARY KEY NOT NULL,
  "name" TEXT NOT NULL,
  province TEXT NOT NULL,
  country_code TEXT NOT NULL,
  area_code VARCHAR(30) NOT NULL,
  ts TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE UNIQUE INDEX idx_cities_area_code ON cities(area_code);
CREATE UNIQUE INDEX idx_cities_province_name ON cities(province,"name");

ALTER TABLE sub_reports ADD COLUMN city_id BIGINT NOT NULL REFERENCES cities(id);

INSERT INTO cities ("name", province, country_code, area_code)
VALUES
('Wonosobo', 'Jawa Tengah', 'Indonesia', 'W318')
;

