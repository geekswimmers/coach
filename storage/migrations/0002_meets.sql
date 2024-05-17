create table if not exists meet (
    id         varchar(32)  primary key,
    name       varchar(100) not null,
    start_date date         not null,
    end_date   date         not null
);

alter table swimmer_time add meet varchar(32) references meet (id);