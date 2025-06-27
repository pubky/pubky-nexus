use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use nexus_common::models::traits::Collection;
use nexus_common::models::user::{Relationship, UserCounts, UserDetails, UserView};
use setup::run_setup;
use std::time::Duration;
use tokio::runtime::Runtime;

mod setup;

const USER_IDS: [&str; 25] = [
    "1t5fyryjggt6kfha18jcstnz3wfyqh9fycra5sg8hr5qhmot5rky",
    "3iwsuz58pgrf7nw4kx8mg3fib1kqyi4oxqmuqxzsau1mpn5weipo",
    "3qgon1apkcmp63xbqpkrb3zzrja3nq9wou4u5bf7uu8rc9ehfo3y",
    "3s88s3b9ik7pg3s4s3u48enp3kbweaydx33fsgd6tnrosaxz6dfy",
    "4b3xhs34k1c8xbem1tj9phr4nf8xkn6w1eckkie3gipmgsfsbw6y",
    "4nacrqeuwh35kwrziy4m376uuyi7czazubgtyog4adm77ayqigxo",
    "4p1qa1ko7wuta4f1qm8io495cqsmefbgfp85wtnm9bj55gqbhjpo",
    "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro",
    "5bem6bm75ku5pqz4em3bcsbm6n5grywja15x3739eesjhsdo6nfy",
    "5ddrprkjm19mz8rokgnqgisommz3zdnfz1yhg1is9kmaoujwrsby",
    "5f4e8eoogmkhqeyo5ijdix3ma6rw9byj8m36yrjp78pnxxc379to",
    "5g3fwnue819wfdjwiwm8qr35ww6uxxgbzrigrtdgmbi19ksioeoy",
    "5nwna1g9kk14tcc59fhp6uk165heafgnng8bhft1m49ioishnawy",
    "6gahxazkp5jk3n69h856gqjoak66xbpybq5c13abnmw3kyhygfty",
    "6ramoshwf43ykn3bdfxb1qn9yy7zbrjyknzrycqxh3s59fapukny",
    "6xejaazm58f5dca3aj6o4is3k55wxy86hyxtd1pu5h897cfq76yy",
    "6z6dsqajktysrzmciep3tt8n8y873ccn4zxney1tmh7k51rw1j5o",
    "77m8jyqqrd41xzu5b67m8z5wuypuatc54gmw5gcax6ae3yeca6wo",
    "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y",
    "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so",
    "7kbjzgcx3xygokesys6jso13tt9u5n995p9q54a1co7cai9ujcso",
    "7oq5wj1adxk1u94ojh6eknwj3b4z88zcbb51dbam5zn7zeqnzoio",
    "7w4hmktqa7gia5thmk7zki8px7ttwpwjtgaaaou4tbqx64re8d1o",
    "8ajb4fbw91fuzywtix3jsc5x416jjpwrue4qricj7k7nt8fjensy",
    "8gmq7a5cpn8bd57co871ob6txx9hamt1q5gqdsyiotgee58dr4dy",
];

fn bench_get_full_by_id(c: &mut Criterion) {
    println!("**********************************************************************************");
    println!("Test the performance of getting a user view by ID, using index or graph as needed");
    println!("**********************************************************************************");

    run_setup();

    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let viewer_id = "5g3fwnue819wfdjwiwm8qr35ww6uxxgbzrigrtdgmbi19ksioeoy"; // Provide the viewer_id
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("get_user_view_by_id", user_id),
        &user_id,
        |b, &id| {
            b.to_async(&rt).iter(|| async {
                let user = UserView::get_by_id(id, Some(viewer_id), None)
                    .await
                    .unwrap();
                std::hint::black_box(user);
            });
        },
    );
}

fn bench_get_relationship_by_id(c: &mut Criterion) {
    println!("*********************************************************************************************************");
    println!("Test the performance of getting a following/follower relationship by ID, using index or graph as needed.");
    println!("*********************************************************************************************************");

    run_setup();

    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let viewer_id = "5g3fwnue819wfdjwiwm8qr35ww6uxxgbzrigrtdgmbi19ksioeoy";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("get_relationship_by_id", user_id),
        &user_id,
        |b, &id| {
            b.to_async(&rt).iter(|| async {
                let relationship = Relationship::get_by_id(id, Some(viewer_id)).await.unwrap();
                std::hint::black_box(relationship);
            });
        },
    );
}

fn bench_get_counts_from_graph(c: &mut Criterion) {
    println!("*********************************************************************************************************");
    println!("Test the performance of getting user counts from the graph database.");
    println!("*********************************************************************************************************");

    run_setup();

    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("get_counts_from_graph", user_id),
        &user_id,
        |b, &id| {
            b.to_async(&rt).iter(|| async {
                let counts = UserCounts::get_from_graph(id).await.unwrap();
                std::hint::black_box(counts);
            });
        },
    );
}

fn bench_get_counts_by_id(c: &mut Criterion) {
    println!("*********************************************************************************************************");
    println!("Test the performance of getting user counts by ID, using index or graph as needed.");
    println!("*********************************************************************************************************");

    run_setup();

    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("get_counts_by_id", user_id),
        &user_id,
        |b, &id| {
            b.to_async(&rt).iter(|| async {
                let counts = UserCounts::get_by_id(id).await.unwrap();
                std::hint::black_box(counts);
            });
        },
    );
}

fn bench_get_details_from_graph(c: &mut Criterion) {
    println!("*********************************************************************************************************");
    println!("Test the performance of getting user details from the graph database.");
    println!("*********************************************************************************************************");

    run_setup();

    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("get_details_from_graph", user_id),
        &user_id,
        |b, &id| {
            b.to_async(&rt).iter(|| async {
                let details = UserDetails::get_from_graph(&[id]).await.unwrap();
                std::hint::black_box(details);
            });
        },
    );
}

fn bench_get_details_by_id(c: &mut Criterion) {
    println!("*********************************************************************************************************");
    println!("Test the performance of getting user details by ID, checking both index and graph.");
    println!("*********************************************************************************************************");

    run_setup();

    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("get_details_by_id", user_id),
        &user_id,
        |b, &id| {
            b.to_async(&rt).iter(|| async {
                let details = UserDetails::get_by_id(id).await.unwrap();
                std::hint::black_box(details);
            });
        },
    );
}

fn bench_get_details_by_ids_list(c: &mut Criterion) {
    println!("*********************************************************************************************************");
    println!("Test the performance of getting user details by a list of IDs, checking both index and graph.");
    println!("*********************************************************************************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("get_details_by_ids_list", "25_users"),
        &USER_IDS,
        |b, &user_ids| {
            b.to_async(&rt).iter(|| async {
                let user_details = UserDetails::get_by_ids(&user_ids).await.unwrap();
                std::hint::black_box(user_details);
            });
        },
    );
}

fn bench_get_details_by_ids_list_from_graph(c: &mut Criterion) {
    println!("*********************************************************************************************************");
    println!(
        "Test the performance of getting user details by a list of IDs from the graph database."
    );
    println!("*********************************************************************************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("get_details_by_ids_list_from_graph", "25_users"),
        &USER_IDS,
        |b, &user_ids| {
            b.to_async(&rt).iter(|| async {
                let user_details = UserDetails::get_by_ids(&user_ids).await.unwrap();
                std::hint::black_box(user_details);
            });
        },
    );
}

fn configure_criterion() -> Criterion {
    Criterion::default()
        .measurement_time(Duration::new(5, 0))
        .sample_size(100)
        .warm_up_time(Duration::new(1, 0))
}

criterion_group! {
    name = benches;
    config = configure_criterion();
    targets = bench_get_full_by_id,
              bench_get_relationship_by_id,
              bench_get_counts_from_graph,
              bench_get_counts_by_id,
              bench_get_details_from_graph,
              bench_get_details_by_id,
              bench_get_details_by_ids_list,
              bench_get_details_by_ids_list_from_graph
}

criterion_main!(benches);
