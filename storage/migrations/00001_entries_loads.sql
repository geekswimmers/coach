create table if not exists entries_load (
    id           serial    primary key,
    load_time    timestamp not null default CURRENT_TIMESTAMP,
    num_swimmers integer   not null,
    num_entries  integer   not null,
    duration     integer   not null,
    swimmers     text      not null
);

create index idx_load_time on entries_load (load_time);