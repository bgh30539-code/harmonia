import { useApp } from "../store";
import { LibraryView } from "../views/LibraryView";
import { AlbumsView } from "../views/AlbumsView";
import { ArtistsView } from "../views/ArtistsView";
import { PlaylistsView } from "../views/PlaylistsView";
import { FavoritesView } from "../views/FavoritesView";
import { RecentView } from "../views/RecentView";
import { MostPlayedView } from "../views/MostPlayedView";
import { SearchView } from "../views/SearchView";
import { AlbumView } from "../views/AlbumView";
import { ArtistView } from "../views/ArtistView";
import { PlaylistView } from "../views/PlaylistView";
import { SettingsView } from "../views/SettingsView";

export function ViewRouter() {
  const { view } = useApp();
  switch (view.name) {
    case "library":
      return <LibraryView key="library" />;
    case "albums":
      return <AlbumsView key="albums" />;
    case "artists":
      return <ArtistsView key="artists" />;
    case "playlists":
      return <PlaylistsView key="playlists" />;
    case "favorites":
      return <FavoritesView key="favorites" />;
    case "recent":
      return <RecentView key="recent" />;
    case "mostPlayed":
      return <MostPlayedView key="mostPlayed" />;
    case "search":
      return <SearchView key={`search-${view.query}`} query={view.query} />;
    case "album":
      return <AlbumView key={`album-${view.title}`} title={view.title} artist={view.artist} />;
    case "artist":
      return <ArtistView key={`artist-${view.artist}`} name={view.artist} />;
    case "playlist":
      return <PlaylistView key={`pl-${view.id}`} id={view.id} name={view.title} />;
    case "settings":
      return <SettingsView key="settings" />;
    default:
      return null;
  }
}
