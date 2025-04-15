#!/bin/bash
# This script continuously sends five different GET requests to Nexus service
# on localhost:8080 and prints the requests per second every 1000 requests.
#
# Endpoints chosen:
# 1. GET /v0/info
# 2. GET /v0/tags/hot with minimal query parameters
# 3. GET /v0/stream/posts
# 4. GET /v0/user/operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo
# 5. GET /v0/post/operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo/0032X0BB0XX5G
# 6. GET /v0/stream/users/username?username=a&viewer_id=operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo
# 7. GET /v0/stream/users?viewer_id=operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo&source=influencers&timeframe=today&limit=30
# 8. GET /v0/stream/posts?source=following&viewer_id=operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo&observer_id=operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo&sorting=total_engagement&skip=0&limit=10&tags=bitcoin


counter=0
start_time=$(date +%s.%N)

while true; do
    # Request 1: Server info
    curl -s http://localhost:8080/v0/info > /dev/null

    # Request 2: Hot tags (minimal params)
    curl -s "http://localhost:8080/v0/tags/hot?skip=0&limit=10" > /dev/null

    # Request 3: Stream posts (no parameters)
    curl -s http://localhost:8080/v0/stream/posts > /dev/null

    # Request 4: User info for a user
    curl -s "http://localhost:8080/v0/user/operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo" > /dev/null

    # Request 5: Post details for a post
    curl -s http://localhost:8080/v0/post/operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo/0032X0BB0XX5G > /dev/null
    
    # Request 6: Steam users by username prefix match
    curl -s "http://localhost:8080/v0/stream/users/username?username=a&viewer_id=operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo" > /dev/null

    # Request 7: Stream popular users for this month
    curl -s "http://localhost:8080/v0/stream/users?viewer_id=operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo&source=influencers&timeframe=today&limit=30" > /dev/null

    # Request 8: Complex stream of posts that will always hit the graph by popularity
    curl -s "http://localhost:8080/v0/stream/posts?source=following&viewer_id=operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo&observer_id=operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo&sorting=total_engagement&skip=0&limit=10&tags=bitcoin" > /dev/null

    counter=$((counter + 8))
    
    # Every 1000 requests, calculate and print the rate.
    if (( counter % 1000 == 0 )); then
        current_time=$(date +%s.%N)
        elapsed=$(echo "$current_time - $start_time" | bc)
        # Calculate requests per second using bc for floating-point arithmetic.
        rps=$(echo "$counter / $elapsed" | bc -l)
        echo "Requests per second: $rps (Total requests: $counter)"
    fi
done
