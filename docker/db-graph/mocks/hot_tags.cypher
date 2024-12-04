// Generate user keys
:param silence_sound => 'pyc598poqkdgtx1wc4aeptx67mqg71mmywyh7uzkffzittjmbiuo';
:param colorful_flower => 'qumq6fady4bmw4w5tpsrj1tg36g3qo4tcfedga9p4bg4so4ikyzy';
:param rocky_mountain => 'r4irb481b8qspaixq1brwre8o87cxybsbk9iwe1f6f9ukrxxs7bo';
:param noisy_river => 'r91hi8kc3x6761gwfiigr7yn6nca1z47wm6jadhw1jbx1co93r9y';
:param juicy_orange => 'tkpeqpx3ywoawiw6q8e6kuo9o3egr7fnhx83rudznbrrmqgdmomo';
:param this_month => 7 * 24 * 60 * 60 * 1000;
:param all => 40 * 24 * 60 * 60 * 1000;

// ################################
// ##### Create Users #############
// ################################
// Timeframe=Today
WITH datetime().epochMillis AS timeframe_millis
MERGE (u:User {id: $silence_sound}) 
SET u.name = "+silence_sound05", 
    u.indexed_at = timeframe_millis;

//Timeframe=ThisMonth
WITH datetime().epochMillis - $this_month AS timeframe_millis
MERGE (u:User {id: $colorful_flower}) 
SET u.name = "+colorful_flower887", 
    u.indexed_at = timeframe_millis;

WITH datetime().epochMillis - $this_month AS timeframe_millis
MERGE (u:User {id: $rocky_mountain}) 
SET u.name = "+rocky_mountain67", 
    u.indexed_at = timeframe_millis;

//Timeframe=All
WITH datetime().epochMillis - $all AS timeframe_millis
MERGE (u:User {id: $noisy_river}) 
SET u.name = "+noisy_river88", 
    u.indexed_at = timeframe_millis;

WITH datetime().epochMillis - $all AS timeframe_millis
MERGE (u:User {id: $juicy_orange}) 
SET u.name = "+juicy_orange998", 
    u.indexed_at = timeframe_millis;