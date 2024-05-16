create table if not exists meet (
    id          varchar(32)  primary key,
    name        varchar(50)  not null,
    start_date  varchar(50)  not null,
    end_date    varchar(10)  not null
);

alter table swimmer_time add meet varchar(32) references meet (id);