use crate::utils::server::TestServiceServer;
use anyhow::Result;
use nexus_common::models::user::UserDetails;
use nexus_common::utils::test_utils::random_pubky_id;

const ALDERT: &str = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
const NUH: &str = "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy";

/// One slot per id, in the ids' order, `None` for an id without a node: the
/// ranking filters and the user view zip on that position. The known ids are
/// requested in descending order, so a read that sorted by id would fail on
/// every run, whatever the random id sorts as.
#[tokio_shared_rt::test(shared)]
async fn test_get_by_ids_keeps_one_slot_per_id_in_order() -> Result<()> {
    let _ = TestServiceServer::get_test_server().await;
    let unknown = random_pubky_id().to_string();

    let details = UserDetails::get_by_ids(&[NUH, &unknown, ALDERT]).await?;

    let ids: Vec<Option<String>> = details
        .iter()
        .map(|details| details.as_ref().map(|details| details.id.to_string()))
        .collect();
    assert_eq!(
        ids,
        vec![Some(NUH.to_string()), None, Some(ALDERT.to_string())]
    );
    Ok(())
}
