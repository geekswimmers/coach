create table if not exists swimmer (
    id          serial       primary key,
    id_external varchar(32)  not null,
    name_first  varchar(50)  not null,
    name_last   varchar(50)  not null,
    gender      varchar(10)  not null,
    birth_date  date         not null
);

create unique index idx_external_id on swimmer (id_external);

create table if not exists swimmer_time (
    id            serial      primary key,
    swimmer       integer     not null references swimmer(id),
    style         varchar(20) not null,
    distance      integer     not null,
    course        varchar(10) not null,
    time_official integer     not null,
    time_date     date        null
);