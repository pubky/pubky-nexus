# Pubky.app Data Model Specification

*Version 0.2.0*

## Introduction

This document specifies the data models and validation rules for the Pubky.app client and homeserver interactions. It defines the structures of data entities, their properties, and the validation rules to ensure data integrity and consistency. This specification is intended for developers who wish to implement their own libraries or clients compatible with Pubky.app.

This document intents to be a faithful representation of our [Rust pubky.app models](https://github.com/pubky/pubky-nexus/tree/main/src/models/pubky_app). If you intend to develop in Rust, use them directly. In case of disagreement between this document and the Rust implementation, the Rust implementation prevails.

## Data Models

### PubkyAppUser

**Description:** Represents a user's profile information.

**URI:** `/pub/pubky.app/profile.json`

**Fields:**

- `name` (string, required): The user's name.
- `bio` (string, optional): A short biography.
- `image` (string, optional): A URL to the user's profile image.
- `links` (array of `UserLink`, optional): A list of links associated with the user.
- `status` (string, optional): The user's current status.

**`UserLink` Object:**

- `title` (string, required): The title of the link.
- `url` (string, required): The URL of the link.

**Validation Rules:**

- **`name`:**
  - Must be at least **3** and at most **50** characters.
  - Cannot be the keyword `[DELETED]`; this is reserved for deleted profiles.

- **`bio`:**
  - Maximum length of **160** characters if provided.

- **`image`:**
  - If provided, must be a valid URL.
  - Maximum length of **300** characters.

- **`links`:**
  - Maximum of **5** links.
  - Each `UserLink` must have:
    - `title`: Maximum length of **100** characters.
    - `url`: Must be a valid URL, maximum length of **300** characters.

- **`status`:**
  - Maximum length of **50** characters if provided.

---

### PubkyAppFile

**Description:** Represents a file uploaded by the user.

**URI:** `/pub/pubky.app/files/:file_id`

**Fields:**

- `name` (string, required): The name of the file.
- `created_at` (integer, required): Timestamp (Unix epoch in seconds) of when the file was created.
- `src` (string, required): The source URL or path of the file.
- `content_type` (string, required): The MIME type of the file.
- `size` (integer, required): The size of the file in bytes.

**Validation Rules:**

- **ID Validation:**
  - The `file_id` in the URI must be a valid **Timestamp ID** (see [ID Generation](#id-generation)).

- **Additional Validation:**
  - Validation for `content_type`, `size`, and other fields should be implemented as needed.

---

### PubkyAppPost

**Description:** Represents a user's post.

**URI:** `/pub/pubky.app/posts/:post_id`

**Fields:**

- `content` (string, required): The content of the post.
- `kind` (string, required): The type of post. Possible values are:
  - `Short`
  - `Long`
  - `Image`
  - `Video`
  - `Link`
  - `File`

- `parent` (string, optional): URI of the parent post if this is a reply.
- `embed` (object, optional): Embedded content.
- `attachments` (array of strings, optional): A list of attachment URIs.

**`embed` Object:**

- `kind` (string, required): Type of the embedded content. Same as `kind` in `PubkyAppPost`.
- `uri` (string, required): URI of the embedded content.

**Validation Rules:**

- **ID Validation:**
  - The `post_id` in the URI must be a valid **Timestamp ID** (see [ID Generation](#id-generation)).

- **`content`:**
  - Must not be the keyword `[DELETED]`; this is reserved for deleted posts.
  - **For `kind` of `Short`:**
    - Maximum length of **1000** characters.
  - **For `kind` of `Long`:**
    - Maximum length of **50000** characters.
  - **For other `kind` values:**
    - Maximum length of **1000** characters.

- **`parent`:**
  - If provided, must be a valid URI.

- **`embed`:**
  - If provided:
    - `uri` must be a valid URI.

- **Additional Validation:**
  - Validation for `attachments` and other fields should be implemented as needed.

---

### PubkyAppTag

**Description:** Represents a tag applied to a URI.

**URI:** `/pub/pubky.app/tags/:tag_id`

**Fields:**

- `uri` (string, required): The URI that is tagged.
- `label` (string, required): The tag label.
- `created_at` (integer, required): Timestamp (Unix epoch in seconds) of when the tag was created.

**Validation Rules:**

- **ID Validation:**
  - The `tag_id` in the URI must be a valid **Hash ID** generated from the `uri` and `label` (see [ID Generation](#id-generation)).

- **`uri`:**
  - Must be a valid URI.

- **`label`:**
  - Must be trimmed and converted to lowercase.
  - Maximum length of **20** characters.

---

### PubkyAppBookmark

**Description:** Represents a bookmark to a URI.

**URI:** `/pub/pubky.app/bookmarks/:bookmark_id`

**Fields:**

- `uri` (string, required): The URI that is bookmarked.
- `created_at` (integer, required): Timestamp (Unix epoch in seconds) of when the bookmark was created.

**Validation Rules:**

- **ID Validation:**
  - The `bookmark_id` in the URI must be a valid **Hash ID** generated from the `uri` (see [ID Generation](#id-generation)).

- **`uri`:**
  - Must be a valid URI.

---

### PubkyAppFollow

**Description:** Represents a follow relationship to another user.

**URI:** `/pub/pubky.app/follows/:user_id`

**Fields:**

- `created_at` (integer, required): Timestamp (Unix epoch in seconds) of when the follow was created.

**Validation Rules:**

- **`created_at`:**
  - Should be validated as needed.

---

### PubkyAppMute

**Description:** Represents a mute relationship to another user.

**URI:** `/pub/pubky.app/mutes/:user_id`

**Fields:**

- `created_at` (integer, required): Timestamp (Unix epoch in seconds) of when the mute was created.

**Validation Rules:**

- **`created_at`:**
  - Should be validated as needed.

---

## Validation Rules

### Common Rules

#### IDs

- **Timestamp IDs**: IDs generated based on the current timestamp, encoded in Crockford Base32.
  - Must be **13** characters long.
  - Decoded ID must represent a valid timestamp after **October 1st, 2024**.
  - Timestamp must not be more than **2 hours** in the future.

- **Hash IDs**: IDs generated by hashing certain fields of the object using Blake3 and encoding in Crockford Base32.
  - For `PubkyAppTag`: Hash of `uri:label`.
  - For `PubkyAppBookmark`: Hash of `uri`.
  - The generated ID must match the provided ID.

### URL Validation

- All URLs must be valid according to standard URL parsing rules.

### String Lengths

- Fields have maximum lengths as specified in their validation rules.

### Content Restrictions

- The content of posts and profiles must not be `[DELETED]`. This keyword is reserved for indicating deleted content.

### Label Formatting

- Labels for tags must be:
  - Trimmed.
  - Converted to lowercase.
  - Maximum length of 20 characters.

--- 

### PubkyAppFeed

**Description:** Represents a feed configuration, allowing users to customize the content they see based on tags, reach, layout, and sort order.

**URI:** `/feeds/:feed_id`

**Fields:**

- `feed` (object, required): The main configuration object for the feed.
  - `tags` (array of strings, optional): Tags used to filter content within the feed.
  - `reach` (string, required): Defines the visibility or scope of the feed. Possible values are:
    - `following`: Content from followed users.
    - `followers`: Content from follower users.
    - `friends`: Content from mutual following users.
    - `all`: Public content accessible to everyone.
  - `layout` (string, required): Specifies the layout of the feed. Options include:
    - `columns`: Organizes feed content in a columnar format.
    - `wide`: Arranges content in a standard wide format.
    - `visual`: Arranges content in visual format.
  - `sort` (string, required): Determines the sorting order of the feed content. Supported values are:
    - `recent`: Most recent content first.
    - `popularity`: Content with the highest engagement.
  - `content` (string, required): Defines the type of content displayed. Options include:
    - `all`: Includes all content types.
    - `posts`: Only posts are shown.
    - `images`: Only media images.
    - `videos`: Only media videos.
    - `links`: Only links.

- `name` (string, required): The user-defined name for this feed configuration.
- `created_at` (integer, required): Timestamp (Unix epoch in milliseconds) representing when the feed was created.

**Validation Rules:**

- **ID Validation:**
  - The `feed_id` in the URI is a **Hash ID** generated from the serialized feed object (the JSON object for `feed`), computed using Blake3 and encoded in Crockford Base32.
  - The generated `feed_id` must match the provided `feed_id`.

---

### PubkyAppLastRead

**Description:** Represents the last read timestamp for notifications, used to track when the user last checked for new activity.

**URI:** `/pub/pubky.app/last_read`

**Fields:**

- `timestamp` (integer, required): Unix epoch time in milliseconds of the last time the user checked notifications.

**Validation Rules:**

- **`timestamp`:** Must be a valid timestamp in milliseconds.

---

## ID Generation

### TimestampId

**Description:** Generates an ID based on the current timestamp.

**Generation Steps:**

1. Obtain the current timestamp in microseconds.
2. Convert the timestamp to an 8-byte big-endian representation.
3. Encode the bytes using Crockford Base32 to get a 13-character ID.

**Validation:**

- The ID must be **13** characters long.
- Decoded timestamp must represent a date after **October 1st, 2024**.
- The timestamp must not be more than **2 hours** in the future.

### HashId

**Description:** Generates an ID based on hashing certain fields of the object.

**Generation Steps:**

1. Concatenate the relevant fields (e.g., `uri:label` for tags).
2. Compute the Blake3 hash of the concatenated string.
3. Take the first half of the hash bytes.
4. Encode the bytes using Crockford Base32.

**Validation:**

- The generated ID must match the provided ID.

---

## Examples

### Example of PubkyAppUser

```json
{
  "name": "Alice",
  "bio": "Blockchain enthusiast and developer.",
  "image": "https://example.com/images/alice.png",
  "links": [
    {
      "title": "GitHub",
      "url": "https://github.com/alice"
    },
    {
      "title": "Website",
      "url": "https://alice.dev"
    }
  ],
  "status": "Exploring the decentralized web."
}
```
### Example of PubkyAppPost

```json
{
  "content": "Hello world! This is my first post.",
  "kind": "short",
  "parent": null,
  "embed": null,
  "attachments": null
}
```

### Example of PubkyAppTag

```json
{
  "uri": "/pub/pubky.app/posts/00321FCW75ZFY",
  "label": "blockchain",
  "created_at": 1700000000
}
```

## Notes
- All timestamps are Unix epoch times in seconds.
- Developers should ensure that all validation rules are enforced to maintain data integrity and interoperability between clients.
- This specification may be updated in future versions to include additional fields or validation rules.

## License
This specification is released under the MIT License.

