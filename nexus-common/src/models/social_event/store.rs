use super::{calendar, Occurrence, SocialContent, StoredEvent};
use crate::db::{exec_single_row, fetch_all_rows_from_graph, fetch_row_from_graph, graph::Query};
use crate::models::{
    error::{ModelError, ModelResult},
    notification::{Notification, NotificationBody},
};
use crate::universal_tag::normalize::{normalize_uri, resource_id};
use chrono::{DateTime, Utc};
use neo4rs::BoltType;
use pubky_social_specs::PubkySocialEventContent;
use std::collections::{BTreeMap, HashMap};

fn json<T: serde::Serialize>(value: &T) -> ModelResult<String> {
    serde_json::to_string(value).map_err(ModelError::from_generic)
}
fn invalid(message: impl ToString) -> ModelError {
    ModelError::InvalidInput(message.to_string())
}
fn normalized(uri: &str) -> ModelResult<String> {
    normalize_uri(uri).map(|(u, _)| u).map_err(invalid)
}
fn post_key(uri: &str) -> Option<crate::models::ResourceKey> {
    use pubky_social_specs::{legacy_v0, ParsedUri, Resource, Visibility};
    if let Ok(ParsedUri {
        user_id,
        visibility: Visibility::Public,
        resource: Resource::Post { id, .. },
    }) = ParsedUri::try_from(uri)
    {
        return Some(crate::models::ResourceKey {
            owner_id: user_id,
            resource_id: id,
        });
    }
    if let Ok(legacy_v0::ParsedUri {
        user_id,
        resource: legacy_v0::Resource::Post(id),
    }) = legacy_v0::ParsedUri::try_from(uri)
    {
        return Some(crate::models::ResourceKey {
            owner_id: user_id,
            resource_id: id,
        });
    }
    None
}

fn map(value: serde_json::Value) -> BoltType {
    match value {
        serde_json::Value::Null => Option::<String>::None.into(),
        serde_json::Value::Bool(v) => v.into(),
        serde_json::Value::Number(v) => v.as_i64().unwrap_or_default().into(),
        serde_json::Value::String(v) => v.into(),
        serde_json::Value::Array(v) => v.into_iter().map(map).collect::<Vec<_>>().into(),
        serde_json::Value::Object(v) => v
            .into_iter()
            .map(|(k, v)| (k, map(v)))
            .collect::<HashMap<_, _>>()
            .into(),
    }
}

/// None is a missing dependency, false is an indexed target of the wrong kind.
pub async fn validate_parent(key: &crate::models::ResourceKey) -> ModelResult<Option<bool>> {
    let row = fetch_row_from_graph(Query::new("event_parent", "MATCH (:User {id:$author})-[:AUTHORED]->(p:Post {id:$id}) RETURN p.kind = 'event' AND NOT coalesce(p.deleted, false) AS valid").param("author",key.owner_id.to_string()).param("id",key.resource_id.clone())).await?;
    row.map(|r| r.get("valid").map_err(Into::into)).transpose()
}

/// Project after the ordinary post write. The derived metadata survives a partial failure,
/// and pending family/aggregate keys are kept until all idempotent follow-up work succeeds.
pub async fn sync(
    author: &str,
    id: &str,
    content: &str,
    parent: Option<&str>,
    value: &SocialContent,
) -> ModelResult<()> {
    let key = parent.and_then(post_key);
    sync_with_parent(author, id, content, parent, key.as_ref(), value).await
}

/// The watcher supplies its resolved native Post key; URI spelling is not identity.
pub async fn sync_with_parent(
    author: &str,
    id: &str,
    content: &str,
    parent: Option<&str>,
    parent_key: Option<&crate::models::ResourceKey>,
    value: &SocialContent,
) -> ModelResult<()> {
    update(author, id, content, parent, parent_key, value, false).await
}
pub async fn remove(author: &str, id: &str) -> ModelResult<()> {
    update(author, id, "", None, None, &SocialContent::Other, true).await
}

async fn update(
    author: &str,
    id: &str,
    content: &str,
    parent: Option<&str>,
    parent_key: Option<&crate::models::ResourceKey>,
    value: &SocialContent,
    deleting: bool,
) -> ModelResult<()> {
    if matches!(value, SocialContent::Other) {
        // Plain posts need no projection writes. Pending keys must survive kind-change
        // retries even after the first attempt has cleared the native metadata.
        let row = fetch_row_from_graph(
            Query::new(
                "social_projection_needed",
                r"
            MATCH (:User {id:$author})-[:AUTHORED]->(p:Post {id:$id})
            WHERE p.social_kind IS NOT NULL OR p.event_content IS NOT NULL
                OR p.social_pending_roots IS NOT NULL OR p.social_pending_ratings IS NOT NULL
            RETURN p.id AS id
        ",
            )
            .param("author", author)
            .param("id", id),
        )
        .await?;
        if row.is_none() {
            return Ok(());
        }
    }
    let now = Utc::now();
    let mut kind = None;
    let mut event = None;
    let mut master = None;
    let mut start_at = None;
    let mut end_at = None;
    let mut status = None;
    let mut policy = None;
    let mut capacity = None;
    let mut all_day = false;
    let mut waitlist = false;
    let mut locations = Vec::new();
    let mut rsvp = None;
    let mut rating = None;
    let mut recurrence = None;
    let mut availability = None;
    let mut availability_target = None;
    let mut guests = 0;
    match value {
        SocialContent::Event(e) => {
            kind = Some("event");
            event = Some(json(e)?);
            master = e.recurrence_of.clone();
            let (s, end) = calendar::timing(e, calendar::local(&e.start).map_err(invalid)?)
                .map_err(invalid)?;
            start_at = Some(s);
            end_at = Some(end);
            status = Some(e.status.wire_name());
            policy = Some(e.attendance.policy.wire_name());
            capacity = e.attendance.capacity;
            all_day = e.show_without_time;
            waitlist = e.attendance.waitlist;
            for location in &e.locations {
                let (uri, scheme) = normalize_uri(&location.uri).map_err(invalid)?;
                locations
                    .push(serde_json::json!({"id":resource_id(&uri),"uri":uri,"scheme":scheme}));
            }
        }
        SocialContent::Rsvp(r) => {
            kind = Some("rsvp");
            rsvp = Some(r.status.wire_name());
            recurrence = r.recurrence_id.clone();
            guests = r.guests;
        }
        SocialContent::Rating(r) => {
            kind = Some("rating");
            rating = Some(r.rating);
        }
        SocialContent::Availability(schedule) => {
            kind = Some("availability");
            availability = Some(json(schedule)?);
            availability_target = Some(normalized(
                schedule
                    .r#for
                    .as_deref()
                    .unwrap_or(&format!("pubky://{author}")),
            )?);
        }
        SocialContent::Other => {}
    }
    let target = if rsvp.is_some() || rating.is_some() {
        Some(normalized(
            parent.ok_or_else(|| invalid("A parent is required"))?,
        )?)
    } else {
        None
    };
    let requires_post = (rating.is_some() || rsvp.is_some()) && parent_key.is_some();
    // Stable canonical key for aggregate recovery even after the target's stored URI changes.
    let rating_key = parent_key
        .map(|key| {
            pubky_social_specs::post_uri_builder(key.owner_id.to_string(), key.resource_id.clone())
        })
        .or_else(|| target.clone());
    let mut fingerprint = blake3::Hasher::new();
    for field in [kind.unwrap_or(""), parent.unwrap_or(""), content] {
        fingerprint.update(&(field.len() as u64).to_le_bytes());
        fingerprint.update(field.as_bytes());
    }
    let fingerprint = fingerprint.finalize().to_hex().to_string();
    let mut query = Query::new("social_projection", r"
        MATCH (u:User {id:$author})-[:AUTHORED]->(p:Post {id:$id})
        WHERE $deleting OR (p.content = $content AND
            (($kind IS NOT NULL AND p.kind=$kind) OR
             ($kind IS NULL AND NOT p.kind IN ['event','rsvp','rating','availability'])))
        OPTIONAL MATCH (:User {id:$target_author})-[:AUTHORED]->(target_post:Post {id:$target_id})
        WITH p,target_post WHERE NOT $requires_post OR target_post IS NOT NULL
        SET p.social_lock = coalesce(p.social_lock, 0) + 1
        WITH p,target_post, CASE WHEN p.event_content IS NOT NULL THEN coalesce(p.event_master,p.uri) ELSE null END AS old_root,
             p.rating_target AS old_rating
        SET p.social_pending_roots = reduce(xs=coalesce(p.social_pending_roots,[]), x IN [old_root, CASE WHEN $event IS NOT NULL THEN coalesce($master,p.uri) ELSE null END] |
            CASE WHEN x IS NULL OR x IN xs THEN xs ELSE xs + x END),
            p.social_pending_ratings = reduce(xs=coalesce(p.social_pending_ratings,[]), x IN [old_rating, CASE WHEN $rating IS NOT NULL THEN $rating_key ELSE null END] |
            CASE WHEN x IS NULL OR x IN xs THEN xs ELSE xs + x END),
            p.social_updated_at = CASE WHEN p.social_fingerprint = $fingerprint THEN p.social_updated_at ELSE $now END,
            p.social_fingerprint = $fingerprint,
            p.social_kind=$kind, p.event_content=$event, p.event_master=$master,
            p.start_at=$start, p.end_at=$end, p.all_day=$all_day, p.status=$status,
            p.policy=$policy, p.capacity=$capacity, p.waitlist=$waitlist,
            p.rsvp_status=$rsvp, p.rsvp_target=CASE WHEN $rsvp IS NULL THEN null ELSE target_post.uri END,
            p.recurrence_id=$recurrence, p.guests=$guests,
            p.availability_content=$availability,p.availability_target=$availability_target,
            p.social_rating=$rating, p.rating_target=CASE WHEN $rating IS NULL THEN null ELSE $rating_key END
        WITH p,target_post
        OPTIONAL MATCH (p)-[edge:LOCATED_AT|RATED]->(previous) DELETE edge
        WITH p,target_post,collect(DISTINCT previous) AS previous_targets
        FOREACH (location IN $locations |
            MERGE (r:Resource {id:location.id})
            ON CREATE SET r.uri=location.uri,r.scheme=location.scheme,r.indexed_at=$now
            MERGE (p)-[:LOCATED_AT]->(r))
        WITH p,target_post,previous_targets
        CALL (p,target_post) {
            WITH p,target_post WHERE $rating IS NOT NULL
            FOREACH (_ IN CASE WHEN target_post IS NULL THEN [] ELSE [1] END | MERGE (p)-[:RATED]->(target_post))
            FOREACH (_ IN CASE WHEN target_post IS NULL THEN [1] ELSE [] END |
                MERGE (r:Resource {id:$resource_id})
                ON CREATE SET r.uri=$target,r.scheme=$scheme,r.indexed_at=$now
                MERGE (p)-[:RATED]->(r))
        }
        FOREACH (r IN [n IN previous_targets WHERE n:Resource AND NOT EXISTS { (n)--() }] | DELETE r)
        RETURN p.social_pending_roots AS roots, p.social_pending_ratings AS ratings
    ")
      .param("author",author).param("id",id).param("content",content).param("deleting",deleting)
      .param("requires_post",requires_post)
      .param("target_author",parent_key.map(|key| key.owner_id.to_string()))
      .param("target_id",parent_key.map(|key| key.resource_id.clone()))
      .param("rating_key",rating_key)
      .param("fingerprint",fingerprint.clone()).param("now",now.timestamp_millis())
      .param("kind",kind).param("event",event).param("master",master)
      .param("start",start_at).param("end",end_at).param("status",status)
      .param("all_day",all_day).param("policy",policy).param("capacity",capacity).param("waitlist",waitlist)
      .param("availability",availability).param("availability_target",availability_target)
      .param("locations",map(serde_json::Value::Array(locations))).param("rsvp",rsvp)
      .param("recurrence",recurrence).param("guests",guests).param("rating",rating).param("target",target.clone());
    let scheme = target
        .as_deref()
        .and_then(|u| u.split_once(':').map(|(s, _)| s))
        .unwrap_or("");
    query = query
        .param("resource_id", target.as_deref().map(resource_id))
        .param("scheme", scheme);
    let Some(row) = fetch_row_from_graph(query).await? else {
        if requires_post {
            if let Some(target) = target {
                // Retry even if the dependency arrives immediately after the query:
                // this attempt did not commit a projection. The watcher re-reads the
                // source, so a concurrently superseded version is safe to retry too.
                return Err(ModelError::MissingDependency(target));
            }
        }
        return Ok(());
    };
    let roots = row.get::<Vec<String>>("roots")?;
    for root in &roots {
        rebuild(root, now).await?;
    }
    for target in row.get::<Vec<String>>("ratings")? {
        refresh_rating(&target).await?;
    }
    for root in &roots {
        flush_family(Some(root)).await?;
    }
    exec_single_row(Query::new("social_projection_complete", r"
        MATCH (:User {id:$author})-[:AUTHORED]->(p:Post {id:$id}) WHERE p.social_fingerprint=$fingerprint
        REMOVE p.social_pending_roots,p.social_pending_ratings
        FOREACH (_ IN CASE WHEN $native THEN [] ELSE [1] END |
            REMOVE p.social_fingerprint,p.social_updated_at,p.social_lock,p.all_day,p.waitlist,p.guests)
    ").param("author",author).param("id",id).param("fingerprint",fingerprint).param("native",kind.is_some())).await?;
    Ok(())
}

async fn refresh_rating(uri: &str) -> ModelResult<()> {
    let key = post_key(uri);
    exec_single_row(
        Query::new(
            "rating_aggregate",
            r"
        CALL () {
            MATCH (:User {id:$target_author})-[:AUTHORED]->(target:Post {id:$target_id}) RETURN target
            UNION
            MATCH (target:Resource {id:$resource_id}) WHERE $target_id IS NULL RETURN target
        }
        SET target.rating_lock=coalesce(target.rating_lock,0)+1
        WITH target
        CALL (target) {
            MATCH (u:User)-[:AUTHORED]->(r:Post)-[:RATED]->(target)
            WHERE r.social_kind='rating' AND NOT coalesce(r.deleted,false)
            WITH u,r ORDER BY r.social_updated_at DESC,r.id DESC
            WITH u,collect(r)[0] AS latest
            RETURN avg(toFloat(latest.social_rating)) AS average,count(*) AS total
        }
        SET target.rating_avg=coalesce(average,0.0),target.rating_count=total
    ",
        )
        .param("target_author", key.as_ref().map(|key| key.owner_id.to_string()))
        .param("target_id", key.as_ref().map(|key| key.resource_id.clone()))
        .param("resource_id", resource_id(uri)),
    )
    .await?;
    Ok(())
}

fn occurrence(
    root: &str,
    key: String,
    post: &StoredEvent,
    event: &PubkySocialEventContent,
    local: chrono::NaiveDateTime,
) -> ModelResult<Occurrence> {
    let (start_at, end_at) = calendar::timing(event, local).map_err(invalid)?;
    let mut locations = event
        .locations
        .iter()
        .map(|l| normalized(&l.uri))
        .collect::<ModelResult<Vec<_>>>()?;
    locations.sort();
    locations.dedup();
    Ok(Occurrence {
        master: root.into(),
        recurrence_id: key,
        post_uri: post.uri.clone(),
        author: post.author.clone(),
        post_id: post.id.clone(),
        start_at,
        end_at,
        status: event.status.wire_name().into(),
        all_day: event.show_without_time,
        locations,
    })
}

async fn rebuild(root: &str, now: DateTime<Utc>) -> ModelResult<()> {
    for _ in 0..3 {
        if rebuild_snapshot(root, now).await? {
            return Ok(());
        }
    }
    Err(ModelError::from_generic(
        "Event family changed during indexing; retry",
    ))
}

async fn rebuild_snapshot(root: &str, now: DateTime<Utc>) -> ModelResult<bool> {
    let rows=fetch_all_rows_from_graph(Query::new("event_family", r"
        MATCH (u:User)-[:AUTHORED]->(p:Post)
        WHERE p.event_content IS NOT NULL AND ((p.uri=$root AND p.event_master IS NULL) OR p.event_master=$root)
        RETURN {uri:p.uri,author:u.id,id:p.id,indexed_at:p.indexed_at,content:p.event_content} AS event
        ORDER BY p.indexed_at,p.id
    ").param("root",root)).await?;
    let posts = rows
        .iter()
        .map(|r| r.get::<StoredEvent>("event").map_err(ModelError::from))
        .collect::<ModelResult<Vec<_>>>()?;
    let events = posts
        .iter()
        .map(|p| {
            serde_json::from_str::<PubkySocialEventContent>(&p.content)
                .map_err(ModelError::from_generic)
        })
        .collect::<ModelResult<Vec<_>>>()?;
    let mut result = BTreeMap::new();
    let mut keys = Vec::new();
    if let Some((post, event)) = posts.iter().zip(&events).find(|(p, _)| p.uri == root) {
        for start in calendar::generated(event).map_err(invalid)? {
            let key = start.format("%Y-%m-%dT%H:%M:%S").to_string();
            keys.push(key.clone());
            result.insert(key.clone(), occurrence(root, key, post, event, start)?);
        }
    }
    let key_set: std::collections::HashSet<&String> = keys.iter().collect();
    // Ascending indexed_at/id means the last override wins deterministically.
    for (post, event) in posts.iter().zip(&events).filter(|(p, _)| p.uri != root) {
        if let Some(key) = event
            .recurrence_id
            .as_ref()
            .filter(|k| key_set.contains(*k))
        {
            result.insert(
                key.clone(),
                occurrence(
                    root,
                    key.clone(),
                    post,
                    event,
                    calendar::local(&event.start).map_err(invalid)?,
                )?,
            );
        } else {
            // An orphan override is an ordinary event, identified by its own URI/start.
            let item = occurrence(
                &post.uri,
                event.start.clone(),
                post,
                event,
                calendar::local(&event.start).map_err(invalid)?,
            )?;
            result.insert(format!("{}:{}", post.uri, event.start), item);
        }
    }
    let window_end = calendar::window_end(now);
    let all_keys = keys;
    let occurrences: Vec<_> = result
        .into_values()
        .filter(|o| o.end_at >= now.timestamp_millis() && o.start_at < window_end)
        .collect();
    // The family snapshot is compared under a per-family graph write lock. A racing source
    // change retries instead of publishing an older calculation over a newer one.
    // Ordered by URI on both sides, so the lock check is one list comparison.
    let mut snapshot = posts
        .iter()
        .map(|p| {
            (
                p.uri.clone(),
                serde_json::json!({"uri":p.uri,"content":p.content}),
            )
        })
        .collect::<Vec<_>>();
    snapshot.sort_by(|a, b| a.0.cmp(&b.0));
    let snapshot = snapshot.into_iter().map(|(_, v)| v).collect::<Vec<_>>();
    let rows = occurrences
        .iter()
        .map(|o| serde_json::to_value(o).unwrap())
        .collect::<Vec<_>>();
    let outcome=fetch_row_from_graph(Query::new("event_occurrences_replace", r"
        MERGE (series:EventSeries {uri:$root})
        SET series.lock=coalesce(series.lock,0)+1
        WITH series
        OPTIONAL MATCH (p:Post) WHERE p.event_content IS NOT NULL AND ((p.uri=$root AND p.event_master IS NULL) OR p.event_master=$root)
        WITH series, p ORDER BY p.uri
        WITH series, [x IN collect({uri:p.uri,content:p.event_content}) WHERE x.uri IS NOT NULL] AS current
        WHERE current = $snapshot
        WITH series,coalesce(series.source,'')<>$source AS source_changed
        SET series.generated_keys=$keys,series.rolled_at=$now,series.source=$source
        WITH series,source_changed
        CALL (series) {
            UNWIND $rows AS item
            OPTIONAL MATCH (old:EventOccurrence {family:$root,master:item.master,recurrence_id:item.recurrence_id})
            WITH series,item,old,
                 old IS NOT NULL AND (old.start_at<>item.start_at OR old.status<>item.status OR old.locations<>item.locations) AS changed
            CALL (item,changed) {
                WITH item WHERE changed
                MATCH (u:User)-[:AUTHORED]->(r:Post)
                WHERE r.social_kind='rsvp' AND r.rsvp_target IN [item.master,item.post_uri]
                  AND (r.recurrence_id IS NULL OR r.recurrence_id=item.recurrence_id)
                  AND NOT coalesce(r.deleted,false)
                WITH item,u,r ORDER BY CASE WHEN r.recurrence_id=item.recurrence_id OR (r.rsvp_target=item.post_uri AND item.post_uri<>item.master) THEN 1 ELSE 0 END DESC,r.social_updated_at DESC,r.id DESC
                WITH item,u,collect(r)[0] AS latest WHERE latest.rsvp_status='accepted'
                MERGE (n:EventNotice {key:$root+'|'+item.recurrence_id+'|'+u.id+'|'+toString($now)})
                SET n.family=$root,n.user_id=u.id,n.event_uri=item.master,n.recurrence_id=item.recurrence_id,
                    n.post_uri=item.post_uri,n.at=$now
            }
            MERGE (o:EventOccurrence {master:item.master,recurrence_id:item.recurrence_id})
            SET o+=item,o.family=$root,o.generation=series.lock
        }
        WITH series,source_changed
        OPTIONAL MATCH (old:EventOccurrence {family:$root}) WHERE old.generation<>series.lock
        CALL (old,source_changed,series) {
            WITH old,series WHERE source_changed AND old IS NOT NULL AND old.end_at >= $now
            MATCH (u:User)-[:AUTHORED]->(r:Post)
            WHERE r.social_kind='rsvp' AND r.rsvp_target IN [old.master,old.post_uri]
              AND (r.recurrence_id IS NULL OR r.recurrence_id=old.recurrence_id)
              AND NOT coalesce(r.deleted,false)
            WITH old,series,u,r ORDER BY CASE WHEN r.recurrence_id=old.recurrence_id OR (r.rsvp_target=old.post_uri AND old.post_uri<>old.master) THEN 1 ELSE 0 END DESC,r.social_updated_at DESC,r.id DESC
            WITH old,series,u,collect(r)[0] AS latest WHERE latest.rsvp_status='accepted'
            MERGE (n:EventNotice {key:$root+'|'+old.recurrence_id+'|'+u.id+'|'+toString(series.lock)})
            SET n.family=$root,n.user_id=u.id,n.event_uri=old.master,n.recurrence_id=old.recurrence_id,n.post_uri=old.post_uri,n.at=$now
        }
        DETACH DELETE old
        RETURN true AS updated
    ").param("root",root).param("now",now.timestamp_millis()).param("keys",all_keys)
      .param("source",json(&snapshot)?).param("snapshot",map(snapshot.into())).param("rows",map(rows.into()))).await?;
    Ok(outcome.is_some())
}

pub async fn flush_notifications() -> ModelResult<()> {
    flush_family(None).await
}
async fn flush_family(family: Option<&str>) -> ModelResult<()> {
    loop {
        let rows=fetch_all_rows_from_graph(Query::new("event_notices", "MATCH (n:EventNotice) WHERE $family IS NULL OR n.family=$family RETURN n.key AS key,n.user_id AS user,n.event_uri AS uri,n.recurrence_id AS recurrence,n.post_uri AS post,n.at AS at ORDER BY n.key LIMIT 100").param("family",family)).await?;
        if rows.is_empty() {
            break;
        }
        for row in rows {
            let notification = Notification {
                timestamp: row.get("at")?,
                body: NotificationBody::EventChanged {
                    event_uri: row.get("uri")?,
                    recurrence_id: row.get("recurrence")?,
                    post_uri: row.get("post")?,
                },
            };
            notification.store_for(&row.get::<String>("user")?).await?;
            exec_single_row(
                Query::new(
                    "event_notice_complete",
                    "MATCH (n:EventNotice {key:$key}) DELETE n",
                )
                .param("key", row.get::<String>("key")?),
            )
            .await?;
        }
    }
    Ok(())
}

pub async fn maintain_occurrences(now: DateTime<Utc>) -> ModelResult<()> {
    let mut after = String::new();
    loop {
        let rows=fetch_all_rows_from_graph(Query::new("event_series_page", "MATCH (s:EventSeries) WHERE s.uri>$after RETURN s.uri AS uri ORDER BY s.uri LIMIT 100").param("after",after.clone())).await?;
        if rows.is_empty() {
            break;
        }
        for row in rows {
            let uri: String = row.get("uri")?;
            rebuild(&uri, now).await?;
            after = uri;
        }
    }
    flush_notifications().await
}
pub async fn prune_orphan_rsvps() -> ModelResult<()> {
    // Drop only the attendance projection, preserving the source reply and its notification.
    // generated_keys spans the full capped lifetime, so rolling windows never prune valid RSVPs.
    // Batched so one run never holds the whole graph's orphans in one transaction.
    loop {
        let row = fetch_row_from_graph(Query::new(
            "rsvp_orphans",
            r"
        MATCH (p:Post) WHERE p.social_kind='rsvp' AND p.recurrence_id IS NOT NULL
        OPTIONAL MATCH (target:Post {uri:p.rsvp_target})
        OPTIONAL MATCH (s:EventSeries {uri:coalesce(target.event_master,target.uri)})
        WITH p,s WHERE s IS NOT NULL AND NOT p.recurrence_id IN coalesce(s.generated_keys,[])
        WITH p LIMIT 1000
        REMOVE p.rsvp_status,p.rsvp_target,p.recurrence_id
        RETURN count(p) AS pruned
    ",
        ))
        .await?;
        let pruned: i64 = row.map(|r| r.get("pruned")).transpose()?.unwrap_or(0);
        if pruned < 1000 {
            return Ok(());
        }
    }
}
