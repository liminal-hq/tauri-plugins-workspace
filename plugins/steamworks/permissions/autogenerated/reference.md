## Default Permission

Default permissions for the steamworks plugin: player info, achievements and stats. Clearing achievements, and anything added later such as presence, overlay and cloud files, is granted separately. Listening to events also needs the core event permission (`core:event:default`).

#### This default permission set includes the following:

- `allow-player-info`
- `allow-achievements`
- `allow-stats`

## Permission Table

<table>
<tr>
<th>Identifier</th>
<th>Description</th>
</tr>


<tr>
<td>

`steamworks:allow-activate-overlay`

</td>
<td>

Enables the activate_overlay command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`steamworks:deny-activate-overlay`

</td>
<td>

Denies the activate_overlay command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`steamworks:allow-activate-overlay-to-web-page`

</td>
<td>

Enables the activate_overlay_to_web_page command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`steamworks:deny-activate-overlay-to-web-page`

</td>
<td>

Denies the activate_overlay_to_web_page command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`steamworks:allow-clear-achievement`

</td>
<td>

Enables the clear_achievement command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`steamworks:deny-clear-achievement`

</td>
<td>

Denies the clear_achievement command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`steamworks:allow-clear-rich-presence`

</td>
<td>

Enables the clear_rich_presence command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`steamworks:deny-clear-rich-presence`

</td>
<td>

Denies the clear_rich_presence command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`steamworks:allow-get-achievement`

</td>
<td>

Enables the get_achievement command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`steamworks:deny-get-achievement`

</td>
<td>

Denies the get_achievement command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`steamworks:allow-get-player`

</td>
<td>

Enables the get_player command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`steamworks:deny-get-player`

</td>
<td>

Denies the get_player command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`steamworks:allow-get-stat`

</td>
<td>

Enables the get_stat command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`steamworks:deny-get-stat`

</td>
<td>

Denies the get_stat command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`steamworks:allow-get-status`

</td>
<td>

Enables the get_status command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`steamworks:deny-get-status`

</td>
<td>

Denies the get_status command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`steamworks:allow-indicate-achievement-progress`

</td>
<td>

Enables the indicate_achievement_progress command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`steamworks:deny-indicate-achievement-progress`

</td>
<td>

Denies the indicate_achievement_progress command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`steamworks:allow-is-available`

</td>
<td>

Enables the is_available command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`steamworks:deny-is-available`

</td>
<td>

Denies the is_available command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`steamworks:allow-set-rich-presence`

</td>
<td>

Enables the set_rich_presence command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`steamworks:deny-set-rich-presence`

</td>
<td>

Denies the set_rich_presence command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`steamworks:allow-set-stat`

</td>
<td>

Enables the set_stat command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`steamworks:deny-set-stat`

</td>
<td>

Denies the set_stat command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`steamworks:allow-store-stats`

</td>
<td>

Enables the store_stats command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`steamworks:deny-store-stats`

</td>
<td>

Denies the store_stats command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`steamworks:allow-unlock-achievement`

</td>
<td>

Enables the unlock_achievement command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`steamworks:deny-unlock-achievement`

</td>
<td>

Denies the unlock_achievement command without any pre-configured scope.

</td>
</tr>

<tr>
<td>

`steamworks:allow-player-info`

</td>
<td>

Reads whether Steam is available, its status, and the signed-in player's name, ID and language.

</td>
</tr>

<tr>
<td>

`steamworks:allow-achievements`

</td>
<td>

Reads and unlocks achievements and shows their progress.

</td>
</tr>

<tr>
<td>

`steamworks:allow-achievements-dev`

</td>
<td>

Re-locks achievements. Meant for development builds, so a shipped app cannot wipe a player's progress.

</td>
</tr>

<tr>
<td>

`steamworks:allow-stats`

</td>
<td>

Reads, writes and stores stats.

</td>
</tr>

<tr>
<td>

`steamworks:allow-presence`

</td>
<td>

Sets and clears rich presence, the status line friends see.

</td>
</tr>

<tr>
<td>

`steamworks:allow-overlay`

</td>
<td>

Opens Steam overlay pages and web pages.

</td>
</tr>
</table>
