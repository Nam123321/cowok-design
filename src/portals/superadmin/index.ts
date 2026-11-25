// @story 14.2
// @implements ADR-008 (Barrel export for lazy-loading SuperAdmin Portal in src/App.jsx)
export {
  SuperAdminPortalApp,
  SuperAdminPortalApp as SuperAdminPortalPage,
  type SuperAdminPortalPageProps,
} from "./SuperAdminPortalApp";
export { default } from "./SuperAdminPortalApp";
