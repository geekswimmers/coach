alter table entries_load rename to import_history;
alter table import_history add dataset varchar(20);

alter table swimmer rename column name_first TO first_name;
alter table swimmer rename column name_last TO last_name;

alter table swimmer_time rename column time_official TO official_time;
alter table swimmer_time rename column time_date TO date_time;