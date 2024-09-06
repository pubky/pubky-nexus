# Pubky skunk-work indexer production migration

We can temporarily develop using as placeholder for Neo4J DB the `skunk.cypher` created with the `skunk-work/scripts/genCypher.js` .

Create a new `.env` file from `.env-sample` and run the following command:

```bash
docker exec neo4j bash /db-graph/run-queries.sh
```
