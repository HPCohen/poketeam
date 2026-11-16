import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  createBrowserRouter,
  Link,
  NavLink,
  Outlet,
  RouterProvider,
  useLocation,
} from "react-router-dom";
import { useEffect, useRef } from "react";
import { ApiError } from "./api/client";
import { SessionProvider, useSession } from "./session";
import { Home, TeamEditor } from "./features/teams";
import { Catalog, PokemonDetail } from "./features/catalog";
import { Counters } from "./features/counters";
import { Notifications } from "./features/notifications";
export function createClient() {
  return new QueryClient({
    defaultOptions: {
      queries: {
        retry: (count, error) =>
          !(error instanceof ApiError && error.status < 500) && count < 1,
        staleTime: 15_000,
        refetchOnWindowFocus: true,
      },
      mutations: { retry: false },
    },
  });
}
function Content() {
  const { epoch } = useSession();
  const location = useLocation();
  const initialNavigation = useRef(true);
  useEffect(() => {
    if (initialNavigation.current) {
      initialNavigation.current = false;
      return;
    }
    document.querySelector("main")?.focus();
  }, [location.pathname]);
  return (
    <main id="main" tabIndex={-1}>
      <Outlet key={epoch} />
    </main>
  );
}
function Layout() {
  return (
    <>
      <a className="skip" href="#main">
        Skip to content
      </a>
      <header>
        <Link className="brand" to="/" aria-label="Poketeam home">
          <span className="brand-mark" aria-hidden="true" />
          poketeam<span className="brand-dot">.</span>
        </Link>
        <nav aria-label="Main navigation">
          <NavLink to="/" end>
            Your teams
          </NavLink>
          <NavLink to="/pokemon">Pokédex</NavLink>
          <NavLink to="/notifications">Changes</NavLink>
        </nav>
      </header>
      <SessionProvider>
        <Content />
      </SessionProvider>
      <footer>
        <span className="brand">poketeam.</span>
        <span>A good team makes all the difference.</span>
        <span>Pokémon belongs to its respective owners.</span>
      </footer>
    </>
  );
}
export const routes = [
  {
    element: <Layout />,
    children: [
      { path: "/", element: <Home /> },
      { path: "/teams/new/*", element: <TeamEditor /> },
      { path: "/teams/:id/counter", element: <Counters /> },
      { path: "/teams/:id/*", element: <TeamEditor /> },
      { path: "/pokemon", element: <Catalog /> },
      { path: "/pokemon/:pokemonId", element: <PokemonDetail /> },
      { path: "/notifications", element: <Notifications /> },
      {
        path: "*",
        element: (
          <section>
            <h1>Page not found</h1>
            <Link to="/">Return home</Link>
          </section>
        ),
      },
    ],
  },
];
export function App() {
  return (
    <QueryClientProvider client={createClient()}>
      <RouterProvider router={createBrowserRouter(routes)} />
    </QueryClientProvider>
  );
}
