import React, { Component, lazy, Suspense } from 'react';
import { Provider } from 'react-redux';
import { Route, Switch, Redirect } from 'react-router';
import { ConnectedRouter } from 'connected-react-router';
import qs from 'query-string';
import localforage from 'localforage';
import * as Sentry from '@sentry/react';

import { CircularProgress, Grid } from '@material-ui/core';

import MyCommaAuth, { config as AuthConfig, storage as AuthStorage } from '@moretore/my-comma-auth';
import { athena as Athena, auth as Auth, billing as Billing, request as Request } from '@moretore/api';

import { getZoom } from './url';
import { isDemo } from './demo';
import store, { history } from './store';

import ErrorFallback from './components/ErrorFallback';

const Explorer = lazy(() => import('./components/explorer'));
const AnonymousLanding = lazy(() => import('./components/anonymous'));

class App extends Component {
  constructor(props) {
    super(props);

    this.state = {
      initialized: false,
    };

    let pairToken;
    if (window.location) {
      pairToken = qs.parse(window.location.search).pair;
    }

    if (pairToken) {
      try {
        localforage.setItem('pairToken', pairToken);
      } catch (err) {
        console.error(err);
      }
    }
  }

  apiErrorResponseCallback(resp) {
    if (resp.status === 401) {
      MyCommaAuth.logOut();
    }
  }

  async componentDidMount() {
    if (typeof window !== 'undefined') {
      console.log('window is defined');
      if (window.location) {
        console.log('window.location is defined');
        if (window.location.pathname === "/v2/auth/") {
          console.log('Pathname matches');
          try {
            const { code, provider } = qs.parse(window.location.search);
            const token = await Auth.refreshAccessToken(code, provider);
            if (token) {
              AuthStorage.setCommaAccessToken(token);
            }
          } catch (err) {
            console.error(err);
            Sentry.captureException(err, { fingerprint: 'app_auth_refresh_token' });
          }
        }
      } else {
        console.log('window.location is undefined');
      }
    } else {
      console.log('window is undefined');
    }


    const token = await MyCommaAuth.init();
    if (token) {
      Request.configure(token, this.apiErrorResponseCallback);
      Billing.configure(token, this.apiErrorResponseCallback);
      Athena.configure(token, this.apiErrorResponseCallback);
    }

    this.setState({ initialized: true });

    // set up analytics, low priority, so we do this last
  }

  redirectLink() {
    let url = '/';
    if (typeof window.sessionStorage !== 'undefined' && sessionStorage.getItem('redirectURL') !== null) {
      url = sessionStorage.getItem('redirectURL');
      sessionStorage.removeItem('redirectURL');
    }
    return url;
  }

  authRoutes() {
    return (
      <Switch>
        <Route path="/auth/">
          <Redirect to={this.redirectLink()} />
        </Route>
        <Route path="/" component={Explorer} />
      </Switch>
    );
  }

  anonymousRoutes() {
    return (
      <Switch>
        <Route path="/auth/">
          <Redirect to="/" />
        </Route>
        <Route path="/" component={AnonymousLanding} />
      </Switch>
    );
  }

  renderLoading() {
    return (
      <Grid container alignItems="center" style={{ width: '100%', height: '100vh' }}>
        <Grid item align="center" xs={12}>
          <CircularProgress size="10vh" style={{ color: '#525E66' }} />
        </Grid>
      </Grid>
    );
  }

  render() {
    if (!this.state.initialized) {
      return this.renderLoading();
    }

    const showLogin = !MyCommaAuth.isAuthenticated() && !isDemo() && !getZoom(window.location.pathname);
    let content = (
      <Suspense fallback={this.renderLoading()}>
        { showLogin ? this.anonymousRoutes() : this.authRoutes() }
      </Suspense>
    );

    // Use ErrorBoundary in production only
    if (import.meta.env.PROD) {
      content = (
        <Sentry.ErrorBoundary fallback={(props) => <ErrorFallback {...props} />}>
          {content}
        </Sentry.ErrorBoundary>
      );
    }

    return (
      <Provider store={store}>
        <ConnectedRouter history={history}>
          {content}
        </ConnectedRouter>
      </Provider>
    );
  }
}

export default App;