import psycopg2
from psycopg2 import sql

class PostgresDB:
    def __init__(self, host, port, dbname, user, password):
        self.host = host
        self.port = port
        self.dbname = dbname
        self.user = user
        self.password = password
        self.connection = None

    def connect(self):
        try:
            self.connection = psycopg2.connect(
                host=self.host,
                port=self.port,
                dbname=self.dbname,
                user=self.user,
                password=self.password
            )
            print("Connection to the database was successful.")
        except (Exception, psycopg2.DatabaseError) as error:
            print(f"Error: {error}")

    def close(self):
        if self.connection is not None:
            self.connection.close()
            print("Database connection closed.")

    def query(self, query, params=None):
        try:
            cursor = self.connection.cursor()
            cursor.execute(query, params)
            result = cursor.fetchall()
            cursor.close()
            return result
        except (Exception, psycopg2.DatabaseError) as error:
            print(f"Error: {error}")
            return None
        
    def query_with_desc(self, query, params=None):
        try:
            cursor = self.connection.cursor()
            cursor.execute(query, params)
            columns = [desc[0] for desc in cursor.description]
            rows = cursor.fetchall()
            cursor.close()
            result = [dict(zip(columns, row)) for row in rows]
            return result
        except (Exception, psycopg2.DatabaseError) as error:
            print(f"Error: {error}")
            return None

if __name__ == "__main__":
    # Example usage
    db = PostgresDB(
        host="154.38.175.6",
        port="5432",
        dbname="connect_development",
        user="loco",
        password="loco"
    )

    db.connect()

    # Example query
    result = db.query("SELECT version();")
    if result:
        print(f"Database version: {result}")

    db.close()
