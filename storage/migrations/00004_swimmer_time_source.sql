alter table swimmer_time add if not exists dataset varchar(20);
drop index if exists idx_swimmer_time;

create unique index if not exists udx_swimmer_time on swimmer_time (swimmer, style, distance, course, time_official, time_date, dataset);