create table if not exists swimmer (
    id          varchar(32)  primary key,
    name_first  varchar(50)  not null,
    name_last   varchar(50)  not null,
    gender      varchar(10)  not null,
    birth_date  date         not null
);

create table if not exists swimmer_time (
    id            serial      primary key,
    swimmer       varchar(32) not null references swimmer(id),
    style         varchar(20) not null,
    distance      integer     not null,
    course        varchar(10) not null,
    time_official integer     not null,
    time_date     date        null
);

create unique index idx_swimmer_time on swimmer_time (swimmer, style, distance, course, time_official, time_date);